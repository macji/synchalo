use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use parking_lot::{Mutex, RwLock};
use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, TransportConfig};
use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use synchalo_core::{
    AppError, ClipboardEvent, DevicePlatform, MAX_CLIPBOARD_BYTES, PROTOCOL_VERSION, content_hash,
};
use synchalo_transfer::{
    FileManifest, TRANSFER_CHUNK_BYTES, prepare_incoming, verify_and_commit_incoming,
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::mpsc,
    time::timeout,
};
use uuid::Uuid;

use crate::{DiscoveredPeer, PairingCodeManager};

const SERVER_NAME: &str = "synchalo.local";
const MAX_WIRE_FRAME_BYTES: usize = 2 * 1024 * 1024;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);
const CONNECTION_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(10);
const AUTH_CLIENT_LABEL: &[u8] = b"synchalo-client-auth-v1";
const AUTH_SERVER_LABEL: &[u8] = b"synchalo-server-auth-v1";

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportCredentials {
    pub certificate_der: Vec<u8>,
    pub private_key_der: Vec<u8>,
    pub signing_key: [u8; 32],
}

impl TransportCredentials {
    pub fn generate() -> Result<Self, AppError> {
        let certified =
            generate_simple_self_signed(vec![SERVER_NAME.to_owned()]).map_err(network_error)?;
        let mut signing_key = [0_u8; 32];
        getrandom::fill(&mut signing_key).map_err(network_error)?;
        Ok(Self {
            certificate_der: certified.cert.der().to_vec(),
            private_key_der: certified.signing_key.serialize_der(),
            signing_key,
        })
    }

    fn signing_key(&self) -> SigningKey {
        SigningKey::from_bytes(&self.signing_key)
    }

    pub fn verifying_key(&self) -> [u8; 32] {
        self.signing_key().verifying_key().to_bytes()
    }

    pub fn certificate_fingerprint(&self) -> String {
        blake3::hash(&self.certificate_der).to_hex().to_string()
    }
}

#[derive(Debug, Clone)]
pub struct TransportIdentity {
    pub device_id: Uuid,
    pub device_name: String,
    pub platform: DevicePlatform,
    pub space_id: Uuid,
    pub credentials: TransportCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrustedPeer {
    pub device_id: Uuid,
    pub device_name: String,
    pub platform: DevicePlatform,
    pub space_id: Uuid,
    pub certificate_der: Vec<u8>,
    pub verifying_key: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairingCandidate {
    pub request_id: Uuid,
    pub device_id: Uuid,
    pub device_name: String,
    pub platform: DevicePlatform,
}

#[derive(Debug, Clone)]
pub enum TransportEvent {
    PairingApprovalRequested(PairingCandidate),
    Paired {
        peer: TrustedPeer,
        adopted_space_id: Uuid,
        incoming: bool,
    },
    PeerOnline {
        device_id: Uuid,
        address: SocketAddr,
    },
    PeerOffline {
        device_id: Uuid,
    },
    ClipboardReceived {
        from_device_id: Uuid,
        event: ClipboardEvent,
    },
    IncomingFileStarted {
        from_device_id: Uuid,
        manifest: FileManifest,
    },
    FileProgress {
        transfer_id: Uuid,
        peer_device_id: Uuid,
        transferred: u64,
        total: u64,
        bytes_per_second: u64,
        incoming: bool,
    },
    FileCompleted {
        transfer_id: Uuid,
        peer_device_id: Uuid,
        path: Option<PathBuf>,
        incoming: bool,
    },
    FileFailed {
        transfer_id: Uuid,
        peer_device_id: Uuid,
        error: String,
        incoming: bool,
    },
    Error(String),
}

#[derive(Clone)]
pub struct LanTransport {
    inner: Arc<TransportInner>,
}

struct TransportInner {
    endpoint: Endpoint,
    identity: RwLock<TransportIdentity>,
    pairing: PairingCodeManager,
    trusted: RwLock<HashMap<Uuid, TrustedPeer>>,
    connections: RwLock<HashMap<Uuid, Connection>>,
    pending_pairings: Mutex<HashMap<Uuid, tokio::sync::oneshot::Sender<bool>>>,
    receive_directory: RwLock<PathBuf>,
    events_tx: mpsc::UnboundedSender<TransportEvent>,
}

impl LanTransport {
    pub fn start(
        identity: TransportIdentity,
        pairing: PairingCodeManager,
        trusted: Vec<TrustedPeer>,
        receive_directory: PathBuf,
        bind_port: u16,
    ) -> Result<(Self, mpsc::UnboundedReceiver<TransportEvent>), AppError> {
        let server_config = server_config(&identity.credentials)?;
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), bind_port);
        let endpoint = Endpoint::server(server_config, bind_addr).map_err(network_error)?;
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let transport = Self {
            inner: Arc::new(TransportInner {
                endpoint,
                identity: RwLock::new(identity),
                pairing,
                trusted: RwLock::new(
                    trusted
                        .into_iter()
                        .map(|peer| (peer.device_id, peer))
                        .collect(),
                ),
                connections: RwLock::new(HashMap::new()),
                pending_pairings: Mutex::new(HashMap::new()),
                receive_directory: RwLock::new(receive_directory),
                events_tx,
            }),
        };
        Ok((transport, events_rx))
    }

    pub fn local_addr(&self) -> Result<SocketAddr, AppError> {
        self.inner.endpoint.local_addr().map_err(network_error)
    }

    pub fn set_space_id(&self, space_id: Uuid) {
        self.inner.identity.write().space_id = space_id;
    }

    pub fn update_device_name(&self, name: String) {
        self.inner.identity.write().device_name = name;
    }

    pub fn set_receive_directory(&self, path: PathBuf) {
        *self.inner.receive_directory.write() = path;
    }

    pub fn add_trusted_peer(&self, peer: TrustedPeer) {
        self.inner.trusted.write().insert(peer.device_id, peer);
    }

    pub fn revoke_peer(&self, id: Uuid) {
        self.inner.trusted.write().remove(&id);
        if let Some(connection) = self.inner.connections.write().remove(&id) {
            connection.close(0_u32.into(), b"device revoked");
        }
    }

    pub fn respond_to_pairing(&self, request_id: Uuid, accepted: bool) -> bool {
        self.inner
            .pending_pairings
            .lock()
            .remove(&request_id)
            .is_some_and(|sender| sender.send(accepted).is_ok())
    }

    pub fn trusted_peer(&self, id: Uuid) -> Option<TrustedPeer> {
        self.inner.trusted.read().get(&id).cloned()
    }

    pub fn online_peer_ids(&self) -> Vec<Uuid> {
        self.inner.connections.read().keys().copied().collect()
    }

    pub async fn pair_with(
        &self,
        discovered: &DiscoveredPeer,
        code: &str,
    ) -> Result<TrustedPeer, AppError> {
        let password: String = code.chars().filter(char::is_ascii_digit).collect();
        if password.len() != 6 {
            return Err(AppError::InvalidInput(
                "pairing code must contain six digits".to_owned(),
            ));
        }
        let identity = self.inner.identity.read().clone();
        let client_wire = WireDevice::from_identity(&identity);
        let (spake, spake_message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(password.as_bytes()),
            &Identity::new(identity.device_id.as_bytes()),
            &Identity::new(discovered.device_id.as_bytes()),
        );

        let connection = timeout(
            HANDSHAKE_TIMEOUT,
            self.inner
                .endpoint
                .connect_with(insecure_client_config()?, discovered.address, SERVER_NAME)
                .map_err(network_error)?,
        )
        .await
        .map_err(|_| AppError::Network("pairing connection timed out".to_owned()))?
        .map_err(network_error)?;
        let (mut send, mut recv) = connection.open_bi().await.map_err(network_error)?;
        write_frame(
            &mut send,
            &WireFrame::PairStart {
                device: client_wire.clone(),
                spake_message,
            },
        )
        .await?;
        let challenge: WireFrame = read_frame(&mut recv).await?;
        let (server_wire, server_spake) = match challenge {
            WireFrame::PairChallenge {
                device,
                spake_message,
            } => (device, spake_message),
            WireFrame::Error { message } => return Err(AppError::Network(message)),
            _ => return Err(protocol_error("expected pairing challenge")),
        };
        if server_wire.device_id != discovered.device_id {
            return Err(protocol_error("discovered device identity changed"));
        }
        let shared_key = spake
            .finish(&server_spake)
            .map_err(|_| AppError::Network("pairing key agreement failed".to_owned()))?;
        let transcript = pairing_transcript(&client_wire, &server_wire)?;
        let client_proof = pairing_proof(&shared_key, b"client", &transcript)?;
        write_frame(
            &mut send,
            &WireFrame::PairProof {
                proof: client_proof,
            },
        )
        .await?;
        let accepted: WireFrame = read_frame(&mut recv).await?;
        let server_proof = match accepted {
            WireFrame::PairAccepted { proof } => proof,
            WireFrame::Error { message } => return Err(AppError::Network(message)),
            _ => return Err(protocol_error("expected pairing acceptance")),
        };
        verify_pairing_proof(&shared_key, b"server", &transcript, &server_proof)?;
        send.finish().map_err(network_error)?;
        connection.close(0_u32.into(), b"pairing complete");

        let peer = server_wire.into_trusted_peer()?;
        let adopted_space_id = peer.space_id;
        self.set_space_id(adopted_space_id);
        self.add_trusted_peer(peer.clone());
        let _ = self.inner.events_tx.send(TransportEvent::Paired {
            peer: peer.clone(),
            adopted_space_id,
            incoming: false,
        });
        self.connect_trusted(peer.device_id, discovered.address)
            .await?;
        Ok(peer)
    }

    pub async fn connect_trusted(
        &self,
        peer_id: Uuid,
        address: SocketAddr,
    ) -> Result<(), AppError> {
        if self.inner.connections.read().contains_key(&peer_id) {
            return Ok(());
        }
        let peer = self
            .trusted_peer(peer_id)
            .ok_or_else(|| AppError::Network("device is not trusted".to_owned()))?;
        let identity = self.inner.identity.read().clone();
        if peer.space_id != identity.space_id {
            return Err(AppError::SyncSpaceMismatch);
        }
        let connection = timeout(
            HANDSHAKE_TIMEOUT,
            self.inner
                .endpoint
                .connect_with(
                    pinned_client_config(&peer.certificate_der)?,
                    address,
                    SERVER_NAME,
                )
                .map_err(network_error)?,
        )
        .await
        .map_err(|_| AppError::Network("device connection timed out".to_owned()))?
        .map_err(network_error)?;
        let mut nonce = [0_u8; 32];
        getrandom::fill(&mut nonce).map_err(network_error)?;
        let signature = identity
            .credentials
            .signing_key()
            .sign(&trusted_client_message(
                identity.device_id,
                peer.device_id,
                identity.space_id,
                &nonce,
            ));
        let (mut send, mut recv) = connection.open_bi().await.map_err(network_error)?;
        write_frame(
            &mut send,
            &WireFrame::TrustedStart {
                device_id: identity.device_id,
                nonce: nonce.to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
        )
        .await?;
        let response: WireFrame = read_frame(&mut recv).await?;
        let (server_nonce, server_signature) = match response {
            WireFrame::TrustedAccepted { nonce, signature } => (nonce, signature),
            WireFrame::Error { message } => return Err(AppError::Network(message)),
            _ => return Err(protocol_error("expected trusted handshake response")),
        };
        verify_signature(
            &peer.verifying_key,
            &trusted_server_message(
                identity.device_id,
                peer.device_id,
                identity.space_id,
                &nonce,
                &server_nonce,
            ),
            &server_signature,
        )?;
        send.finish().map_err(network_error)?;
        self.register_connection(peer_id, connection);
        Ok(())
    }

    pub async fn broadcast_clipboard(&self, event: ClipboardEvent) -> Result<usize, AppError> {
        let peer_ids: Vec<_> = self.inner.connections.read().keys().copied().collect();
        let mut delivered = 0;
        for peer_id in peer_ids {
            match self.send_clipboard_to(peer_id, event.clone()).await {
                Ok(()) => delivered += 1,
                Err(error) => {
                    tracing::debug!(%peer_id, %error, "clipboard delivery failed");
                }
            }
        }
        Ok(delivered)
    }

    pub async fn send_clipboard_to(
        &self,
        peer_id: Uuid,
        event: ClipboardEvent,
    ) -> Result<(), AppError> {
        let connection = self
            .inner
            .connections
            .read()
            .get(&peer_id)
            .cloned()
            .ok_or_else(|| AppError::Network("target device is offline".to_owned()))?;
        let signature = self
            .inner
            .identity
            .read()
            .credentials
            .signing_key()
            .sign(&clipboard_signing_bytes(&event)?);
        let message = WireFrame::Clipboard {
            event,
            signature: signature.to_bytes().to_vec(),
        };
        if let Err(error) = send_application_message(&connection, &message).await {
            self.inner.connections.write().remove(&peer_id);
            let _ = self
                .inner
                .events_tx
                .send(TransportEvent::PeerOffline { device_id: peer_id });
            return Err(error);
        }
        Ok(())
    }

    pub async fn send_file(
        &self,
        peer_id: Uuid,
        path: &Path,
        manifest: FileManifest,
    ) -> Result<(), AppError> {
        let connection = self
            .inner
            .connections
            .read()
            .get(&peer_id)
            .cloned()
            .ok_or_else(|| AppError::Network("target device is offline".to_owned()))?;
        let metadata = tokio::fs::symlink_metadata(path)
            .await
            .map_err(network_error)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() != manifest.file_size
        {
            return Err(AppError::File(
                "source file changed before transfer".to_owned(),
            ));
        }
        let (mut send, mut recv) = connection.open_bi().await.map_err(network_error)?;
        write_frame(
            &mut send,
            &WireFrame::FileOffer {
                manifest: manifest.clone(),
            },
        )
        .await?;
        let resume_offset = match read_frame::<WireFrame>(&mut recv).await? {
            WireFrame::FileAccept { resume_offset } if resume_offset <= manifest.file_size => {
                resume_offset
            }
            WireFrame::FileFailed { message } => return Err(AppError::File(message)),
            _ => return Err(protocol_error("expected file acceptance")),
        };
        let mut file = tokio::fs::File::open(path).await.map_err(network_error)?;
        file.seek(std::io::SeekFrom::Start(resume_offset))
            .await
            .map_err(network_error)?;
        let mut transferred = resume_offset;
        let started = Instant::now();
        let mut last_report = Instant::now() - Duration::from_secs(1);
        let mut buffer = vec![0_u8; TRANSFER_CHUNK_BYTES];
        while transferred < manifest.file_size {
            let remaining = (manifest.file_size - transferred) as usize;
            let read = file
                .read(&mut buffer[..remaining.min(TRANSFER_CHUNK_BYTES)])
                .await
                .map_err(network_error)?;
            if read == 0 {
                return Err(AppError::File(
                    "source file ended during transfer".to_owned(),
                ));
            }
            send.write_all(&buffer[..read])
                .await
                .map_err(network_error)?;
            transferred += read as u64;
            if last_report.elapsed() >= Duration::from_millis(150)
                || transferred == manifest.file_size
            {
                let speed = ((transferred - resume_offset) as f64
                    / started.elapsed().as_secs_f64().max(0.001))
                    as u64;
                let _ = self.inner.events_tx.send(TransportEvent::FileProgress {
                    transfer_id: manifest.id,
                    peer_device_id: peer_id,
                    transferred,
                    total: manifest.file_size,
                    bytes_per_second: speed,
                    incoming: false,
                });
                last_report = Instant::now();
            }
        }
        send.finish().map_err(network_error)?;
        match read_frame::<WireFrame>(&mut recv).await? {
            WireFrame::FileCompleted => {
                let _ = self.inner.events_tx.send(TransportEvent::FileCompleted {
                    transfer_id: manifest.id,
                    peer_device_id: peer_id,
                    path: None,
                    incoming: false,
                });
                Ok(())
            }
            WireFrame::FileFailed { message } => {
                let _ = self.inner.events_tx.send(TransportEvent::FileFailed {
                    transfer_id: manifest.id,
                    peer_device_id: peer_id,
                    error: message.clone(),
                    incoming: false,
                });
                Err(AppError::File(message))
            }
            _ => Err(protocol_error("expected file completion")),
        }
    }

    pub async fn run(&self) {
        while let Some(incoming) = self.inner.endpoint.accept().await {
            let transport = self.clone();
            tauri_independent_spawn(async move {
                match incoming.await {
                    Ok(connection) => {
                        if let Err(error) = transport.handle_incoming_connection(connection).await {
                            let _ = transport
                                .inner
                                .events_tx
                                .send(TransportEvent::Error(error.to_string()));
                        }
                    }
                    Err(error) => {
                        let _ = transport
                            .inner
                            .events_tx
                            .send(TransportEvent::Error(error.to_string()));
                    }
                }
            });
        }
    }

    async fn handle_incoming_connection(&self, connection: Connection) -> Result<(), AppError> {
        let (mut send, mut recv) = timeout(HANDSHAKE_TIMEOUT, connection.accept_bi())
            .await
            .map_err(|_| AppError::Network("incoming handshake timed out".to_owned()))?
            .map_err(network_error)?;
        let request: WireFrame = read_frame(&mut recv).await?;
        match request {
            WireFrame::PairStart {
                device,
                spake_message,
            } => {
                self.handle_incoming_pairing(connection, send, recv, device, spake_message)
                    .await
            }
            WireFrame::TrustedStart {
                device_id,
                nonce,
                signature,
            } => {
                self.handle_incoming_trusted(connection, &mut send, device_id, nonce, signature)
                    .await
            }
            _ => {
                write_frame(
                    &mut send,
                    &WireFrame::Error {
                        message: "invalid handshake".to_owned(),
                    },
                )
                .await?;
                Err(protocol_error("invalid handshake"))
            }
        }
    }

    async fn handle_incoming_pairing(
        &self,
        connection: Connection,
        mut send: SendStream,
        mut recv: RecvStream,
        client_wire: WireDevice,
        client_spake_message: Vec<u8>,
    ) -> Result<(), AppError> {
        let code =
            self.inner.pairing.begin_network_attempt()?.ok_or_else(|| {
                AppError::Network("this device is not accepting pairing".to_owned())
            })?;
        let server_identity = self.inner.identity.read().clone();
        let server_wire = WireDevice::from_identity(&server_identity);
        let (spake, server_spake_message) = Spake2::<Ed25519Group>::start_b(
            &Password::new(code.as_bytes()),
            &Identity::new(client_wire.device_id.as_bytes()),
            &Identity::new(server_identity.device_id.as_bytes()),
        );
        let shared_key = spake
            .finish(&client_spake_message)
            .map_err(|_| AppError::Network("pairing key agreement failed".to_owned()))?;
        write_frame(
            &mut send,
            &WireFrame::PairChallenge {
                device: server_wire.clone(),
                spake_message: server_spake_message,
            },
        )
        .await?;
        let proof_frame: WireFrame = read_frame(&mut recv).await?;
        let proof = match proof_frame {
            WireFrame::PairProof { proof } => proof,
            _ => return Err(protocol_error("expected client pairing proof")),
        };
        let transcript = pairing_transcript(&client_wire, &server_wire)?;
        verify_pairing_proof(&shared_key, b"client", &transcript, &proof)?;
        let pending_peer = client_wire.clone().into_trusted_peer()?;
        let request_id = Uuid::new_v4();
        let candidate = PairingCandidate {
            request_id,
            device_id: pending_peer.device_id,
            device_name: pending_peer.device_name.clone(),
            platform: pending_peer.platform,
        };
        let (approval_tx, approval_rx) = tokio::sync::oneshot::channel();
        self.inner
            .pending_pairings
            .lock()
            .insert(request_id, approval_tx);
        if self
            .inner
            .events_tx
            .send(TransportEvent::PairingApprovalRequested(candidate))
            .is_err()
        {
            self.inner.pending_pairings.lock().remove(&request_id);
            return Err(AppError::Network(
                "pairing approval UI is unavailable".to_owned(),
            ));
        }
        let accepted = timeout(Duration::from_secs(30), approval_rx)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or(false);
        self.inner.pending_pairings.lock().remove(&request_id);
        if !accepted {
            self.inner.pairing.invalidate();
            write_frame(
                &mut send,
                &WireFrame::Error {
                    message: "pairing request was rejected or timed out".to_owned(),
                },
            )
            .await?;
            send.finish().map_err(network_error)?;
            let _ = timeout(Duration::from_secs(1), send.stopped()).await;
            connection.close(1_u32.into(), b"pairing rejected");
            return Ok(());
        }
        if !self.inner.pairing.consume_active_code(&code) {
            return Err(AppError::Network("pairing code expired".to_owned()));
        }
        let server_proof = pairing_proof(&shared_key, b"server", &transcript)?;
        write_frame(
            &mut send,
            &WireFrame::PairAccepted {
                proof: server_proof,
            },
        )
        .await?;
        send.finish().map_err(network_error)?;
        let mut peer = pending_peer;
        peer.space_id = server_identity.space_id;
        self.add_trusted_peer(peer.clone());
        let _ = self.inner.events_tx.send(TransportEvent::Paired {
            peer,
            adopted_space_id: server_identity.space_id,
            incoming: true,
        });
        let _ = timeout(Duration::from_secs(2), send.stopped()).await;
        connection.close(0_u32.into(), b"pairing complete");
        Ok(())
    }

    async fn handle_incoming_trusted(
        &self,
        connection: Connection,
        send: &mut SendStream,
        device_id: Uuid,
        client_nonce: Vec<u8>,
        client_signature: Vec<u8>,
    ) -> Result<(), AppError> {
        if client_nonce.len() != 32 {
            return Err(protocol_error("invalid authentication nonce"));
        }
        let peer = self
            .trusted_peer(device_id)
            .ok_or_else(|| AppError::Network("device is not trusted".to_owned()))?;
        let identity = self.inner.identity.read().clone();
        if peer.space_id != identity.space_id {
            return Err(AppError::SyncSpaceMismatch);
        }
        verify_signature(
            &peer.verifying_key,
            &trusted_client_message(
                peer.device_id,
                identity.device_id,
                identity.space_id,
                &client_nonce,
            ),
            &client_signature,
        )?;
        let mut server_nonce = vec![0_u8; 32];
        getrandom::fill(&mut server_nonce).map_err(network_error)?;
        let signature = identity
            .credentials
            .signing_key()
            .sign(&trusted_server_message(
                peer.device_id,
                identity.device_id,
                identity.space_id,
                &client_nonce,
                &server_nonce,
            ));
        write_frame(
            send,
            &WireFrame::TrustedAccepted {
                nonce: server_nonce,
                signature: signature.to_bytes().to_vec(),
            },
        )
        .await?;
        send.finish().map_err(network_error)?;
        self.register_connection(device_id, connection);
        Ok(())
    }

    fn register_connection(&self, peer_id: Uuid, connection: Connection) {
        if let Some(previous) = self
            .inner
            .connections
            .write()
            .insert(peer_id, connection.clone())
        {
            previous.close(0_u32.into(), b"replaced by newer connection");
        }
        let _ = self.inner.events_tx.send(TransportEvent::PeerOnline {
            device_id: peer_id,
            address: connection.remote_address(),
        });

        let transport = self.clone();
        tauri_independent_spawn(async move {
            while let Ok((send, recv)) = connection.accept_bi().await {
                let transport = transport.clone();
                tauri_independent_spawn(async move {
                    if let Err(error) = transport
                        .handle_application_stream(peer_id, send, recv)
                        .await
                    {
                        let _ = transport
                            .inner
                            .events_tx
                            .send(TransportEvent::Error(error.to_string()));
                    }
                });
            }
            let should_remove = transport
                .inner
                .connections
                .read()
                .get(&peer_id)
                .is_some_and(|current| current.stable_id() == connection.stable_id());
            if should_remove {
                transport.inner.connections.write().remove(&peer_id);
                let _ = transport
                    .inner
                    .events_tx
                    .send(TransportEvent::PeerOffline { device_id: peer_id });
            }
        });
    }

    async fn handle_application_stream(
        &self,
        peer_id: Uuid,
        mut send: SendStream,
        mut recv: RecvStream,
    ) -> Result<(), AppError> {
        let message: WireFrame = read_frame(&mut recv).await?;
        match message {
            WireFrame::Clipboard { event, signature } => {
                if event.origin_device_id != peer_id {
                    return Err(protocol_error("clipboard origin does not match connection"));
                }
                if event.space_id != self.inner.identity.read().space_id {
                    return Err(protocol_error("clipboard sync space does not match"));
                }
                if event.content.is_empty() || event.content.len() > MAX_CLIPBOARD_BYTES {
                    return Err(protocol_error("clipboard payload size is invalid"));
                }
                if content_hash(event.content.as_bytes()) != event.content_hash {
                    return Err(protocol_error("clipboard content hash mismatch"));
                }
                let peer = self
                    .trusted_peer(peer_id)
                    .ok_or_else(|| AppError::Network("device is no longer trusted".to_owned()))?;
                verify_signature(
                    &peer.verifying_key,
                    &clipboard_signing_bytes(&event)?,
                    &signature,
                )?;
                let _ = self
                    .inner
                    .events_tx
                    .send(TransportEvent::ClipboardReceived {
                        from_device_id: peer_id,
                        event,
                    });
                write_frame(&mut send, &WireFrame::Ack).await?;
                send.finish().map_err(network_error)?;
                Ok(())
            }
            WireFrame::FileOffer { manifest } => {
                self.receive_file(peer_id, manifest, send, recv).await
            }
            _ => Err(protocol_error("unsupported application message")),
        }
    }

    async fn receive_file(
        &self,
        peer_id: Uuid,
        manifest: FileManifest,
        mut send: SendStream,
        mut recv: RecvStream,
    ) -> Result<(), AppError> {
        let receive_directory = self.inner.receive_directory.read().clone();
        let target = match prepare_incoming(&receive_directory, &manifest).await {
            Ok(target) => target,
            Err(error) => {
                write_frame(
                    &mut send,
                    &WireFrame::FileFailed {
                        message: error.to_string(),
                    },
                )
                .await?;
                send.finish().map_err(network_error)?;
                return Err(error);
            }
        };
        let _ = self
            .inner
            .events_tx
            .send(TransportEvent::IncomingFileStarted {
                from_device_id: peer_id,
                manifest: manifest.clone(),
            });
        write_frame(
            &mut send,
            &WireFrame::FileAccept {
                resume_offset: target.resume_offset,
            },
        )
        .await?;
        if target.already_complete {
            write_frame(&mut send, &WireFrame::FileCompleted).await?;
            send.finish().map_err(network_error)?;
            let _ = self.inner.events_tx.send(TransportEvent::FileCompleted {
                transfer_id: manifest.id,
                peer_device_id: peer_id,
                path: Some(target.final_path),
                incoming: true,
            });
            return Ok(());
        }

        let mut output = tokio::fs::OpenOptions::new()
            .create(true)
            .append(target.resume_offset > 0)
            .truncate(target.resume_offset == 0)
            .write(true)
            .open(&target.temp_path)
            .await
            .map_err(network_error)?;
        let mut transferred = target.resume_offset;
        let started = Instant::now();
        let mut last_report = Instant::now() - Duration::from_secs(1);
        let mut buffer = vec![0_u8; TRANSFER_CHUNK_BYTES];
        while transferred < manifest.file_size {
            let remaining = (manifest.file_size - transferred) as usize;
            let read = match recv
                .read(&mut buffer[..remaining.min(TRANSFER_CHUNK_BYTES)])
                .await
            {
                Ok(Some(read)) if read > 0 => read,
                Ok(_) => {
                    let error =
                        "file stream ended early; the partial file can be resumed".to_owned();
                    let _ = self.inner.events_tx.send(TransportEvent::FileFailed {
                        transfer_id: manifest.id,
                        peer_device_id: peer_id,
                        error: error.clone(),
                        incoming: true,
                    });
                    return Err(AppError::File(error));
                }
                Err(source) => {
                    let error = format!("file stream interrupted: {source}");
                    let _ = self.inner.events_tx.send(TransportEvent::FileFailed {
                        transfer_id: manifest.id,
                        peer_device_id: peer_id,
                        error: error.clone(),
                        incoming: true,
                    });
                    return Err(AppError::File(error));
                }
            };
            if let Err(source) = output.write_all(&buffer[..read]).await {
                let error = format!("failed to write incoming file: {source}");
                let _ = self.inner.events_tx.send(TransportEvent::FileFailed {
                    transfer_id: manifest.id,
                    peer_device_id: peer_id,
                    error: error.clone(),
                    incoming: true,
                });
                return Err(AppError::File(error));
            }
            transferred += read as u64;
            if last_report.elapsed() >= Duration::from_millis(150)
                || transferred == manifest.file_size
            {
                let speed = ((transferred - target.resume_offset) as f64
                    / started.elapsed().as_secs_f64().max(0.001))
                    as u64;
                let _ = self.inner.events_tx.send(TransportEvent::FileProgress {
                    transfer_id: manifest.id,
                    peer_device_id: peer_id,
                    transferred,
                    total: manifest.file_size,
                    bytes_per_second: speed,
                    incoming: true,
                });
                last_report = Instant::now();
            }
        }
        output.flush().await.map_err(network_error)?;
        output.sync_all().await.map_err(network_error)?;
        drop(output);

        match verify_and_commit_incoming(&target, &manifest).await {
            Ok(path) => {
                write_frame(&mut send, &WireFrame::FileCompleted).await?;
                send.finish().map_err(network_error)?;
                let _ = self.inner.events_tx.send(TransportEvent::FileCompleted {
                    transfer_id: manifest.id,
                    peer_device_id: peer_id,
                    path: Some(path),
                    incoming: true,
                });
                Ok(())
            }
            Err(error) => {
                let _ = tokio::fs::remove_file(&target.temp_path).await;
                write_frame(
                    &mut send,
                    &WireFrame::FileFailed {
                        message: error.to_string(),
                    },
                )
                .await?;
                send.finish().map_err(network_error)?;
                let _ = self.inner.events_tx.send(TransportEvent::FileFailed {
                    transfer_id: manifest.id,
                    peer_device_id: peer_id,
                    error: error.to_string(),
                    incoming: true,
                });
                Err(error)
            }
        }
    }
}

impl Drop for TransportInner {
    fn drop(&mut self) {
        self.endpoint.close(0_u32.into(), b"application shutdown");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireDevice {
    device_id: Uuid,
    device_name: String,
    platform: DevicePlatform,
    space_id: Uuid,
    protocol_version: u16,
    certificate_der: Vec<u8>,
    verifying_key: [u8; 32],
}

impl WireDevice {
    fn from_identity(identity: &TransportIdentity) -> Self {
        Self {
            device_id: identity.device_id,
            device_name: identity.device_name.clone(),
            platform: identity.platform,
            space_id: identity.space_id,
            protocol_version: PROTOCOL_VERSION,
            certificate_der: identity.credentials.certificate_der.clone(),
            verifying_key: identity.credentials.verifying_key(),
        }
    }

    fn into_trusted_peer(self) -> Result<TrustedPeer, AppError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(AppError::Network(format!(
                "protocol version {} is not supported",
                self.protocol_version
            )));
        }
        if self.certificate_der.len() > 64 * 1024 {
            return Err(protocol_error("certificate is too large"));
        }
        VerifyingKey::from_bytes(&self.verifying_key)
            .map_err(|_| protocol_error("invalid device verification key"))?;
        Ok(TrustedPeer {
            device_id: self.device_id,
            device_name: self.device_name,
            platform: self.platform,
            space_id: self.space_id,
            certificate_der: self.certificate_der,
            verifying_key: self.verifying_key,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WireFrame {
    PairStart {
        device: WireDevice,
        spake_message: Vec<u8>,
    },
    PairChallenge {
        device: WireDevice,
        spake_message: Vec<u8>,
    },
    PairProof {
        proof: Vec<u8>,
    },
    PairAccepted {
        proof: Vec<u8>,
    },
    TrustedStart {
        device_id: Uuid,
        nonce: Vec<u8>,
        signature: Vec<u8>,
    },
    TrustedAccepted {
        nonce: Vec<u8>,
        signature: Vec<u8>,
    },
    Clipboard {
        event: ClipboardEvent,
        signature: Vec<u8>,
    },
    FileOffer {
        manifest: FileManifest,
    },
    FileAccept {
        resume_offset: u64,
    },
    FileCompleted,
    FileFailed {
        message: String,
    },
    Ack,
    Error {
        message: String,
    },
}

async fn send_application_message(
    connection: &Connection,
    message: &WireFrame,
) -> Result<(), AppError> {
    let (mut send, mut recv) = connection.open_bi().await.map_err(network_error)?;
    write_frame(&mut send, message).await?;
    send.finish().map_err(network_error)?;
    match read_frame::<WireFrame>(&mut recv).await? {
        WireFrame::Ack => Ok(()),
        WireFrame::Error { message } => Err(AppError::Network(message)),
        _ => Err(protocol_error("expected acknowledgement")),
    }
}

async fn write_frame<T: Serialize>(send: &mut SendStream, value: &T) -> Result<(), AppError> {
    let bytes = serde_json::to_vec(value).map_err(network_error)?;
    if bytes.len() > MAX_WIRE_FRAME_BYTES {
        return Err(protocol_error("wire frame is too large"));
    }
    send.write_u32(bytes.len() as u32)
        .await
        .map_err(network_error)?;
    send.write_all(&bytes).await.map_err(network_error)
}

async fn read_frame<T: DeserializeOwned>(recv: &mut RecvStream) -> Result<T, AppError> {
    let length = recv.read_u32().await.map_err(network_error)? as usize;
    if length == 0 || length > MAX_WIRE_FRAME_BYTES {
        return Err(protocol_error("wire frame length is invalid"));
    }
    let mut bytes = vec![0_u8; length];
    recv.read_exact(&mut bytes).await.map_err(network_error)?;
    serde_json::from_slice(&bytes).map_err(network_error)
}

fn server_config(credentials: &TransportCredentials) -> Result<quinn::ServerConfig, AppError> {
    let certificate = CertificateDer::from(credentials.certificate_der.clone());
    let private_key = PrivatePkcs8KeyDer::from(credentials.private_key_der.clone());
    let mut config = quinn::ServerConfig::with_single_cert(vec![certificate], private_key.into())
        .map_err(network_error)?;
    config.transport_config(connection_transport_config()?);
    Ok(config)
}

fn pinned_client_config(certificate_der: &[u8]) -> Result<ClientConfig, AppError> {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from(certificate_der.to_vec()))
        .map_err(network_error)?;
    let mut config =
        ClientConfig::with_root_certificates(Arc::new(roots)).map_err(network_error)?;
    config.transport_config(connection_transport_config()?);
    Ok(config)
}

fn insecure_client_config() -> Result<ClientConfig, AppError> {
    let crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_no_client_auth();
    let quic = QuicClientConfig::try_from(crypto).map_err(network_error)?;
    let mut config = ClientConfig::new(Arc::new(quic));
    config.transport_config(connection_transport_config()?);
    Ok(config)
}

fn connection_transport_config() -> Result<Arc<TransportConfig>, AppError> {
    let mut config = TransportConfig::default();
    config
        .max_idle_timeout(Some(
            CONNECTION_IDLE_TIMEOUT.try_into().map_err(network_error)?,
        ))
        .keep_alive_interval(Some(KEEP_ALIVE_INTERVAL));
    Ok(Arc::new(config))
}

#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn pairing_transcript(client: &WireDevice, server: &WireDevice) -> Result<Vec<u8>, AppError> {
    serde_json::to_vec(&("synchalo-pair-v1", client, server)).map_err(network_error)
}

fn pairing_proof(shared_key: &[u8], role: &[u8], transcript: &[u8]) -> Result<Vec<u8>, AppError> {
    let mut mac = HmacSha256::new_from_slice(shared_key).map_err(network_error)?;
    mac.update(role);
    mac.update(transcript);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn verify_pairing_proof(
    shared_key: &[u8],
    role: &[u8],
    transcript: &[u8],
    proof: &[u8],
) -> Result<(), AppError> {
    let mut mac = HmacSha256::new_from_slice(shared_key).map_err(network_error)?;
    mac.update(role);
    mac.update(transcript);
    mac.verify_slice(proof)
        .map_err(|_| AppError::Network("pairing proof did not match".to_owned()))
}

fn trusted_client_message(
    client_id: Uuid,
    server_id: Uuid,
    space_id: Uuid,
    nonce: &[u8],
) -> Vec<u8> {
    join_auth_message(AUTH_CLIENT_LABEL, client_id, server_id, space_id, &[nonce])
}

fn trusted_server_message(
    client_id: Uuid,
    server_id: Uuid,
    space_id: Uuid,
    client_nonce: &[u8],
    server_nonce: &[u8],
) -> Vec<u8> {
    join_auth_message(
        AUTH_SERVER_LABEL,
        client_id,
        server_id,
        space_id,
        &[client_nonce, server_nonce],
    )
}

fn join_auth_message(
    label: &[u8],
    client_id: Uuid,
    server_id: Uuid,
    space_id: Uuid,
    pieces: &[&[u8]],
) -> Vec<u8> {
    let mut message =
        Vec::with_capacity(label.len() + 48 + pieces.iter().map(|p| p.len()).sum::<usize>());
    message.extend_from_slice(label);
    message.extend_from_slice(client_id.as_bytes());
    message.extend_from_slice(server_id.as_bytes());
    message.extend_from_slice(space_id.as_bytes());
    for piece in pieces {
        message.extend_from_slice(piece);
    }
    message
}

fn clipboard_signing_bytes(event: &ClipboardEvent) -> Result<Vec<u8>, AppError> {
    serde_json::to_vec(&("synchalo-clipboard-v1", event)).map_err(network_error)
}

fn verify_signature(key: &[u8; 32], message: &[u8], signature: &[u8]) -> Result<(), AppError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| AppError::Network("invalid device verification key".to_owned()))?;
    let signature = Signature::from_slice(signature)
        .map_err(|_| AppError::Network("invalid device signature".to_owned()))?;
    key.verify(message, &signature)
        .map_err(|_| AppError::Network("device signature verification failed".to_owned()))
}

fn tauri_independent_spawn(future: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(future);
}

fn protocol_error(message: impl Into<String>) -> AppError {
    AppError::Network(format!("protocol error: {}", message.into()))
}

fn network_error(error: impl std::fmt::Display) -> AppError {
    AppError::Network(error.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tokio::time::timeout;

    use super::*;

    fn identity(name: &str) -> TransportIdentity {
        TransportIdentity {
            device_id: Uuid::new_v4(),
            device_name: name.to_owned(),
            platform: DevicePlatform::Linux,
            space_id: Uuid::new_v4(),
            credentials: TransportCredentials::generate().unwrap(),
        }
    }

    #[tokio::test]
    async fn pairs_then_authenticates_and_delivers_signed_clipboard() {
        let server_identity = identity("Server");
        let server_id = server_identity.device_id;
        let server_space = server_identity.space_id;
        let server_pairing = PairingCodeManager::new();
        let code = server_pairing.generate(Duration::from_secs(60)).unwrap();
        let server_files = tempfile::tempdir().unwrap();
        let (server, mut server_events) = LanTransport::start(
            server_identity,
            server_pairing,
            Vec::new(),
            server_files.path().to_owned(),
            0,
        )
        .unwrap();
        tokio::spawn({
            let server = server.clone();
            async move { server.run().await }
        });

        let client_identity = identity("Client");
        let client_id = client_identity.device_id;
        let client_files = tempfile::tempdir().unwrap();
        let (client, _client_events) = LanTransport::start(
            client_identity,
            PairingCodeManager::new(),
            Vec::new(),
            client_files.path().to_owned(),
            0,
        )
        .unwrap();
        tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        let discovered = DiscoveredPeer {
            device_id: server_id,
            device_name: "Server".to_owned(),
            platform: DevicePlatform::Linux,
            address: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                server.local_addr().unwrap().port(),
            ),
            pairing_open: true,
            protocol_version: PROTOCOL_VERSION,
            fullname: "server._synchalo._udp.local.".to_owned(),
        };

        let pair_task = tokio::spawn({
            let client = client.clone();
            let code = code.code.clone();
            let discovered = discovered.clone();
            async move { client.pair_with(&discovered, &code).await }
        });
        let approval = timeout(Duration::from_secs(3), server_events.recv())
            .await
            .unwrap()
            .unwrap();
        let request_id = match approval {
            TransportEvent::PairingApprovalRequested(candidate) => {
                assert_eq!(candidate.device_id, client_id);
                candidate.request_id
            }
            other => panic!("expected approval request, got {other:?}"),
        };
        assert!(server.respond_to_pairing(request_id, true));
        let peer = pair_task.await.unwrap().unwrap();
        assert_eq!(peer.device_id, server_id);
        assert_eq!(peer.space_id, server_space);

        let mut saw_pair = false;
        let mut saw_online = false;
        for _ in 0..4 {
            let event = timeout(Duration::from_secs(3), server_events.recv())
                .await
                .unwrap()
                .unwrap();
            match event {
                TransportEvent::Paired { peer, .. } if peer.device_id == client_id => {
                    saw_pair = true
                }
                TransportEvent::PeerOnline { device_id, .. } if device_id == client_id => {
                    saw_online = true
                }
                _ => {}
            }
            if saw_pair && saw_online {
                break;
            }
        }
        assert!(saw_pair && saw_online);

        let text = "hello from client".to_owned();
        let event = ClipboardEvent {
            id: Uuid::now_v7(),
            space_id: server_space,
            origin_device_id: client_id,
            origin_sequence: 1,
            created_at: Utc::now(),
            hlc: synchalo_core::HlcTimestamp {
                physical_ms: Utc::now().timestamp_millis(),
                logical: 0,
            },
            content_hash: content_hash(text.as_bytes()),
            content: text.clone(),
        };
        assert_eq!(client.broadcast_clipboard(event).await.unwrap(), 1);

        loop {
            let received = timeout(Duration::from_secs(3), server_events.recv())
                .await
                .unwrap()
                .unwrap();
            if let TransportEvent::ClipboardReceived { event, .. } = received {
                assert_eq!(event.content, text);
                break;
            }
        }

        let source = client_files.path().join("payload.bin");
        tokio::fs::write(&source, b"verified file payload")
            .await
            .unwrap();
        let (manifest, _) = synchalo_transfer::inspect_file(&source).await.unwrap();
        let transfer_id = manifest.id;
        if let Err(error) = client.send_file(server_id, &source, manifest).await {
            let mut diagnostics = vec![error.to_string()];
            while let Ok(event) = server_events.try_recv() {
                diagnostics.push(format!("{event:?}"));
            }
            panic!("file transfer failed: {diagnostics:#?}");
        }
        loop {
            let received = timeout(Duration::from_secs(3), server_events.recv())
                .await
                .unwrap()
                .unwrap();
            if let TransportEvent::FileCompleted {
                transfer_id: completed_id,
                path: Some(path),
                incoming: true,
                ..
            } = received
            {
                assert_eq!(completed_id, transfer_id);
                assert_eq!(
                    tokio::fs::read(path).await.unwrap(),
                    b"verified file payload"
                );
                break;
            }
        }
    }
}
