interface SwitchProps {
  checked: boolean;
  label: string;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}
export function Switch({ checked, label, disabled, onChange }: SwitchProps) {
  return (
    <button
      aria-checked={checked}
      aria-label={label}
      className="switch"
      disabled={disabled}
      onClick={() => onChange(!checked)}
      role="switch"
      type="button"
    >
      <span className="switch-thumb" />
    </button>
  );
}
