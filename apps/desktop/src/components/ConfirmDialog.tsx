import { AlertTriangle } from "lucide-react";
import { useRef } from "react";

import { ModalDialog } from "./ModalDialog";

export interface ConfirmState {
  title: string;
  body: string;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void | Promise<void>;
  onCancel?: () => void | Promise<void>;
}

interface ConfirmDialogProps {
  state: ConfirmState | null;
  onClose: () => void;
}

export function ConfirmDialog({ state, onClose }: ConfirmDialogProps) {
  const confirmRef = useRef<HTMLButtonElement>(null);

  if (!state) return null;
  const cancel = () => {
    void state.onCancel?.();
    onClose();
  };
  return (
    <ModalDialog
      actions={
        <>
          <button className="button button--secondary" onClick={cancel} type="button">
            取消
          </button>
          <button
            className={`button ${state.danger ? "button--danger" : "button--primary"}`}
            onClick={() => {
              void state.onConfirm();
              onClose();
            }}
            ref={confirmRef}
            type="button"
          >
            {state.confirmLabel}
          </button>
        </>
      }
      className="confirm-dialog"
      initialFocusRef={confirmRef}
      onClose={cancel}
      title={state.title}
    >
      <div className="confirm-message">
        <div className={`dialog-symbol ${state.danger ? "is-danger" : ""}`}>
          <AlertTriangle aria-hidden="true" size={21} />
        </div>
        <p>{state.body}</p>
      </div>
    </ModalDialog>
  );
}
