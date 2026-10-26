import type { ReactNode } from 'react';

interface IconProps {
  className?: string;
}

function iconBase(className: string | undefined, children: ReactNode) {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className ?? 'h-5 w-5'}
    >
      {children}
    </svg>
  );
}

export function HomeIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M3 10.5 12 3l9 7.5" />
      <path d="M5 9.5V21h14V9.5" />
      <path d="M9.5 21v-6h5v6" />
    </>,
  );
}

export function LibraryIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M4 4v16" />
      <path d="M9 4v16" />
      <path d="m13.5 5 5.5 14" />
    </>,
  );
}

export function SearchIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <circle cx="11" cy="11" r="6.5" />
      <path d="m16 16 4.5 4.5" />
    </>,
  );
}

export function PlayIcon({ className }: IconProps) {
  return iconBase(className, <path d="M7 4.5 19 12 7 19.5Z" fill="currentColor" stroke="none" />);
}

export function PauseIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <rect x="6" y="4.5" width="4" height="15" rx="1" fill="currentColor" stroke="none" />
      <rect x="14" y="4.5" width="4" height="15" rx="1" fill="currentColor" stroke="none" />
    </>,
  );
}

export function NextIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M6 5.5 16 12 6 18.5Z" fill="currentColor" stroke="none" />
      <path d="M18 5v14" />
    </>,
  );
}

export function PreviousIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M18 5.5 8 12l10 6.5Z" fill="currentColor" stroke="none" />
      <path d="M6 5v14" />
    </>,
  );
}

export function ShuffleIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M3 7h3l5 10h4" />
      <path d="m16 7h5v5" />
      <path d="m13 7h4l4 5" />
      <path d="M3 17h3l2-4" />
      <path d="m16 17h5v-5" />
    </>,
  );
}

export function RepeatIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M4 9V7a3 3 0 0 1 3-3h10" />
      <path d="m14 1 3 3-3 3" transform="translate(0 2)" />
      <path d="M20 15v2a3 3 0 0 1-3 3H7" />
      <path d="m10 17-3 3 3 3" transform="translate(0 2)" />
    </>,
  );
}

export function RepeatOneIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M4 9V7a3 3 0 0 1 3-3h10" />
      <path d="m14 1 3 3-3 3" transform="translate(0 2)" />
      <path d="M20 15v2a3 3 0 0 1-3 3H7" />
      <path d="m10 17-3 3 3 3" transform="translate(0 2)" />
      <path d="M12 10v4" />
      <path d="m10.5 11.5 1.5-1.5" />
    </>,
  );
}

export function VolumeIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M4 9.5h3L12 5v14L7 14.5H4Z" fill="currentColor" stroke="none" />
      <path d="M15.5 8.5a5 5 0 0 1 0 7" />
      <path d="M18 6a8.5 8.5 0 0 1 0 12" />
    </>,
  );
}

export function VolumeMuteIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M4 9.5h3L12 5v14L7 14.5H4Z" fill="currentColor" stroke="none" />
      <path d="m16 9 5 6" />
      <path d="m21 9-5 6" />
    </>,
  );
}

export function QueueIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M4 6h11" />
      <path d="M4 12h11" />
      <path d="M4 18h7" />
      <path d="m16 14 4 3-4 3Z" fill="currentColor" stroke="none" />
    </>,
  );
}

export function CloseIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="m6 6 12 12" />
      <path d="M18 6 6 18" />
    </>,
  );
}

export function ChevronUpIcon({ className }: IconProps) {
  return iconBase(className, <path d="m6 15 6-6 6 6" />);
}

export function ChevronDownIcon({ className }: IconProps) {
  return iconBase(className, <path d="m6 9 6 6 6-6" />);
}

export function TrashIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M4 7h16" />
      <path d="M9 7V5h6v2" />
      <path d="M6 7l1 13h10l1-13" />
    </>,
  );
}

export function PlaylistIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M4 6h12" />
      <path d="M4 12h12" />
      <path d="M4 18h8" />
      <path d="M18 11v8" />
      <path d="M15.5 16.5h5" />
    </>,
  );
}

export function HeartIcon({ className, filled = false }: IconProps & { filled?: boolean }) {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      viewBox="0 0 24 24"
      fill={filled ? 'currentColor' : 'none'}
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className ?? 'h-5 w-5'}
    >
      <path d="M12 20.5S3.5 15 3.5 9.2A4.7 4.7 0 0 1 12 6a4.7 4.7 0 0 1 8.5 3.2c0 5.8-8.5 11.3-8.5 11.3Z" />
    </svg>
  );
}

export function PlusIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M12 5v14" />
      <path d="M5 12h14" />
    </>,
  );
}

export function EditIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M4 20h4l11-11-4-4L4 16Z" />
      <path d="m13.5 5.5 4 4" />
    </>,
  );
}

export function LogoutIcon({ className }: IconProps) {
  return iconBase(
    className,
    <>
      <path d="M14 4h5v16h-5" />
      <path d="M10 8l-4 4 4 4" />
      <path d="M6 12h9" />
    </>,
  );
}
