interface HaloMarkProps {
  size?: number;
}
export function HaloMark({ size = 28 }: HaloMarkProps) {
  return (
    <svg
      aria-hidden="true"
      className="halo-mark"
      height={size}
      viewBox="0 0 32 32"
      width={size}
    >
      <circle cx="16" cy="16" fill="none" r="11" stroke="currentColor" strokeWidth="3" />
      <circle cx="16" cy="16" fill="currentColor" r="3.2" />
      <path d="M4.7 12.6h4.1M23.2 19.4h4.1" stroke="currentColor" strokeLinecap="round" strokeWidth="2" />
    </svg>
  );
}
