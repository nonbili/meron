// Inline icons for things lucide-react doesn't ship. They mirror lucide's
// 24x24 / stroke-width-2 / currentColor conventions so they sit interchangeably
// alongside lucide icons (e.g. via `fileIconFor`).

export interface IconProps {
  size?: number
  className?: string
}

/** PDF document icon (lucide has no file-pdf): file outline + "PDF" label. */
export function PdfIcon({ size = 16, className }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <path d="M14 2v6h6" />
      <text
        x="12"
        y="18"
        textAnchor="middle"
        fontSize="7"
        fontWeight="700"
        fill="currentColor"
        stroke="none"
        fontFamily="ui-sans-serif, system-ui, sans-serif"
      >
        PDF
      </text>
    </svg>
  )
}
