import type { ArtworkSize } from '@zuno/shared-types';
import { useSession } from '../app/AuthProvider';

interface ArtworkProps {
  artworkId: string | null;
  alt: string;
  size?: ArtworkSize;
  className?: string;
  rounded?: string;
}

export function Artwork({
  artworkId,
  alt,
  size = 'thumb',
  className = 'h-12 w-12',
  rounded = 'rounded-lg',
}: ArtworkProps) {
  const { client } = useSession();

  if (!artworkId) {
    return (
      <div
        aria-hidden="true"
        className={`${className} ${rounded} flex shrink-0 items-center justify-center border border-line/60 bg-gradient-to-br from-accent-blue/25 via-accent-indigo/20 to-accent-violet/25`}
      >
        <span className="text-[0.6rem] font-semibold tracking-widest text-copy-muted uppercase">
          Zuno
        </span>
      </div>
    );
  }

  return (
    <img
      src={client.artworkUrl(artworkId, size)}
      alt={alt}
      loading="lazy"
      decoding="async"
      className={`${className} ${rounded} shrink-0 border border-line/60 object-cover`}
    />
  );
}
