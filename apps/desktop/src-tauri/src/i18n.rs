use synchalo_core::LanguagePreference;

#[derive(Clone, Copy)]
pub enum NativeText {
    TrayOpen,
    TrayPause,
    TraySendFile,
    TrayQuit,
    SelectReceiveFolder,
    SelectFiles,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Locale {
    En,
    ZhCn,
    ZhTw,
    Ja,
    Ko,
}

pub fn text(preference: LanguagePreference, key: NativeText) -> &'static str {
    match (resolve(preference), key) {
        (Locale::En, NativeText::TrayOpen) => "Open SyncHalo",
        (Locale::En, NativeText::TrayPause) => "Pause or resume sync",
        (Locale::En, NativeText::TraySendFile) => "Send files…",
        (Locale::En, NativeText::TrayQuit) => "Quit",
        (Locale::En, NativeText::SelectReceiveFolder) => "Choose receive folder",
        (Locale::En, NativeText::SelectFiles) => "Choose files to sync",
        (Locale::ZhCn, NativeText::TrayOpen) => "打开 SyncHalo",
        (Locale::ZhCn, NativeText::TrayPause) => "暂停或恢复同步",
        (Locale::ZhCn, NativeText::TraySendFile) => "发送文件…",
        (Locale::ZhCn, NativeText::TrayQuit) => "退出",
        (Locale::ZhCn, NativeText::SelectReceiveFolder) => "选择接收目录",
        (Locale::ZhCn, NativeText::SelectFiles) => "选择要同步的文件",
        (Locale::ZhTw, NativeText::TrayOpen) => "開啟 SyncHalo",
        (Locale::ZhTw, NativeText::TrayPause) => "暫停或恢復同步",
        (Locale::ZhTw, NativeText::TraySendFile) => "傳送檔案…",
        (Locale::ZhTw, NativeText::TrayQuit) => "結束",
        (Locale::ZhTw, NativeText::SelectReceiveFolder) => "選擇接收資料夾",
        (Locale::ZhTw, NativeText::SelectFiles) => "選擇要同步的檔案",
        (Locale::Ja, NativeText::TrayOpen) => "SyncHalo を開く",
        (Locale::Ja, NativeText::TrayPause) => "同期を一時停止または再開",
        (Locale::Ja, NativeText::TraySendFile) => "ファイルを送信…",
        (Locale::Ja, NativeText::TrayQuit) => "終了",
        (Locale::Ja, NativeText::SelectReceiveFolder) => "受信フォルダーを選択",
        (Locale::Ja, NativeText::SelectFiles) => "同期するファイルを選択",
        (Locale::Ko, NativeText::TrayOpen) => "SyncHalo 열기",
        (Locale::Ko, NativeText::TrayPause) => "동기화 일시 정지 또는 재개",
        (Locale::Ko, NativeText::TraySendFile) => "파일 보내기…",
        (Locale::Ko, NativeText::TrayQuit) => "종료",
        (Locale::Ko, NativeText::SelectReceiveFolder) => "수신 폴더 선택",
        (Locale::Ko, NativeText::SelectFiles) => "동기화할 파일 선택",
    }
}

pub fn update_notification(
    preference: LanguagePreference,
    ready: bool,
    version: &str,
) -> (&'static str, String) {
    match (resolve(preference), ready) {
        (Locale::En, true) => (
            "SyncHalo update downloaded",
            format!("SyncHalo {version} has been verified. Open the app to install and restart."),
        ),
        (Locale::En, false) => (
            "SyncHalo update available",
            format!("SyncHalo {version} is available. Open the app to view the release notes."),
        ),
        (Locale::ZhCn, true) => (
            "SyncHalo 更新已下载",
            format!("SyncHalo {version} 已完成验证，打开应用安装并重启。"),
        ),
        (Locale::ZhCn, false) => (
            "SyncHalo 发现新版本",
            format!("SyncHalo {version} 可用，打开应用查看发布说明。"),
        ),
        (Locale::ZhTw, true) => (
            "SyncHalo 更新已下載",
            format!("SyncHalo {version} 已完成驗證，開啟應用程式以安裝並重新啟動。"),
        ),
        (Locale::ZhTw, false) => (
            "SyncHalo 有新版本",
            format!("SyncHalo {version} 可供使用，開啟應用程式以檢視版本說明。"),
        ),
        (Locale::Ja, true) => (
            "SyncHalo アップデートをダウンロードしました",
            format!(
                "SyncHalo {version} の検証が完了しました。アプリを開いてインストールし、再起動してください。"
            ),
        ),
        (Locale::Ja, false) => (
            "SyncHalo の新しいバージョンがあります",
            format!(
                "SyncHalo {version} を利用できます。アプリを開いてリリースノートを確認してください。"
            ),
        ),
        (Locale::Ko, true) => (
            "SyncHalo 업데이트 다운로드 완료",
            format!(
                "SyncHalo {version} 검증이 완료되었습니다. 앱을 열어 설치하고 다시 시작하세요."
            ),
        ),
        (Locale::Ko, false) => (
            "SyncHalo 새 버전 사용 가능",
            format!("SyncHalo {version}을 사용할 수 있습니다. 앱을 열어 릴리스 노트를 확인하세요."),
        ),
    }
}

pub fn transfer_notification(
    preference: LanguagePreference,
    incoming: bool,
    file_name: &str,
) -> (&'static str, String) {
    match (resolve(preference), incoming) {
        (Locale::En, true) => (
            "SyncHalo file sync complete",
            format!("{file_name} was saved to the receive folder"),
        ),
        (Locale::En, false) => (
            "SyncHalo file sync complete",
            format!("{file_name} was sent successfully"),
        ),
        (Locale::ZhCn, true) => (
            "SyncHalo 文件同步完成",
            format!("{file_name} 已保存到接收目录"),
        ),
        (Locale::ZhCn, false) => ("SyncHalo 文件同步完成", format!("{file_name} 已发送完成")),
        (Locale::ZhTw, true) => (
            "SyncHalo 檔案同步完成",
            format!("{file_name} 已儲存到接收資料夾"),
        ),
        (Locale::ZhTw, false) => ("SyncHalo 檔案同步完成", format!("{file_name} 已傳送完成")),
        (Locale::Ja, true) => (
            "SyncHalo ファイル同期完了",
            format!("{file_name} を受信フォルダーに保存しました"),
        ),
        (Locale::Ja, false) => (
            "SyncHalo ファイル同期完了",
            format!("{file_name} を送信しました"),
        ),
        (Locale::Ko, true) => (
            "SyncHalo 파일 동기화 완료",
            format!("{file_name} 파일을 수신 폴더에 저장했습니다"),
        ),
        (Locale::Ko, false) => (
            "SyncHalo 파일 동기화 완료",
            format!("{file_name} 파일을 전송했습니다"),
        ),
    }
}

fn resolve(preference: LanguagePreference) -> Locale {
    match preference {
        LanguagePreference::System => system_locale(),
        LanguagePreference::En => Locale::En,
        LanguagePreference::ZhCn => Locale::ZhCn,
        LanguagePreference::ZhTw => Locale::ZhTw,
        LanguagePreference::Ja => Locale::Ja,
        LanguagePreference::Ko => Locale::Ko,
    }
}

fn system_locale() -> Locale {
    static SYSTEM_LOCALE: std::sync::OnceLock<Locale> = std::sync::OnceLock::new();
    *SYSTEM_LOCALE.get_or_init(detect_system_locale)
}

fn detect_system_locale() -> Locale {
    for tag in system_language_tags() {
        if let Some(locale) = locale_from_tag(&tag) {
            return locale;
        }
    }
    Locale::En
}

fn locale_from_tag(tag: &str) -> Option<Locale> {
    let normalized = tag.trim().to_ascii_lowercase().replace('_', "-");
    if normalized.starts_with("zh") {
        if normalized.contains("hant")
            || normalized.contains("-tw")
            || normalized.contains("-hk")
            || normalized.contains("-mo")
        {
            return Some(Locale::ZhTw);
        }
        return Some(Locale::ZhCn);
    }
    if normalized.starts_with("ja") {
        return Some(Locale::Ja);
    }
    if normalized.starts_with("ko") {
        return Some(Locale::Ko);
    }
    normalized.starts_with("en").then_some(Locale::En)
}

fn system_language_tags() -> Vec<String> {
    #[cfg(target_os = "macos")]
    if let Ok(output) = std::process::Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        && output.status.success()
    {
        let tags: Vec<_> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| {
                line.trim()
                    .trim_matches(['(', ')', '"', ',', ' '])
                    .to_owned()
            })
            .filter(|line| !line.is_empty())
            .collect();
        if !tags.is_empty() {
            return tags;
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = std::process::Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[cultureinfo]::CurrentUICulture.Name",
            ])
            .creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = command.output()
            && output.status.success()
        {
            let tag = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !tag.is_empty() {
                return vec![tag];
            }
        }
    }

    ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .flat_map(|value| {
            value
                .split(':')
                .map(|tag| tag.split('.').next().unwrap_or(tag).to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_language_tags_are_resolved() {
        assert_eq!(locale_from_tag("en-US"), Some(Locale::En));
        assert_eq!(locale_from_tag("zh-Hans-CN"), Some(Locale::ZhCn));
        assert_eq!(locale_from_tag("zh_Hant_TW"), Some(Locale::ZhTw));
        assert_eq!(locale_from_tag("ja-JP"), Some(Locale::Ja));
        assert_eq!(locale_from_tag("ko-KR"), Some(Locale::Ko));
        assert_eq!(locale_from_tag("fr-FR"), None);
    }

    #[test]
    fn explicit_preferences_localize_native_text() {
        assert_eq!(text(LanguagePreference::En, NativeText::TrayQuit), "Quit");
        assert_eq!(
            text(LanguagePreference::ZhTw, NativeText::SelectFiles),
            "選擇要同步的檔案"
        );
        assert_eq!(
            text(LanguagePreference::Ja, NativeText::TrayOpen),
            "SyncHalo を開く"
        );
        assert_eq!(
            text(LanguagePreference::Ko, NativeText::TraySendFile),
            "파일 보내기…"
        );
    }
}
