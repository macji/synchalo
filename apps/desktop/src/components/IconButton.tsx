import type { ButtonHTMLAttributes, ReactNode } from "react";

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  icon: ReactNode;
  tone?: "neutral" | "danger" | "accent";
}
export function IconButton({ label, icon, tone = "neutral", className = "", ...props }: IconButtonProps) {
  return (
    <button
      aria-label={label}
      className={`icon-button icon-button--${tone} ${className}`.trim()}
      title={label}
      type="button"
      {...props}
    >
      {icon}
    </button>
  );
}
