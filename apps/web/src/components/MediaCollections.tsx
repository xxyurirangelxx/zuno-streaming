import type { AlbumSummary, ArtistSummary } from '@zuno/shared-types';
import { Artwork } from './Artwork';
import { FavoriteButton } from './FavoriteButton';

export function AlbumGrid({ albums }: { albums: AlbumSummary[] }) {
  if (albums.length === 0) {
    return <p className="px-1 py-6 text-sm text-copy-muted">Nenhum álbum encontrado.</p>;
  }

  return (
    <ul className="grid grid-cols-2 gap-4 sm:grid-cols-3 xl:grid-cols-4">
      {albums.map((album) => (
        <li
          key={album.id}
          className="rounded-zuno border border-line/60 bg-surface/50 p-3 transition-colors hover:border-accent-blue/60"
        >
          <article>
            <Artwork
              artworkId={album.artwork_id}
              alt={album.title}
              size="card"
              className="aspect-square w-full"
              rounded="rounded-xl"
            />
            <div className="mt-2 flex items-start justify-between gap-2">
              <p className="min-w-0 flex-1 truncate text-sm font-semibold text-copy">
                {album.title}
              </p>
              <FavoriteButton kind="album" entity={album} label={album.title} />
            </div>
            <p className="truncate text-xs text-copy-muted">
              {album.artist?.name ?? 'Artista desconhecido'}
              {album.release_year !== null ? ` · ${album.release_year}` : ''}
            </p>
            <p className="mt-1 text-xs text-copy-muted">
              {album.track_count} {album.track_count === 1 ? 'faixa' : 'faixas'}
            </p>
          </article>
        </li>
      ))}
    </ul>
  );
}

export function ArtistList({ artists }: { artists: ArtistSummary[] }) {
  if (artists.length === 0) {
    return <p className="px-1 py-6 text-sm text-copy-muted">Nenhum artista encontrado.</p>;
  }

  return (
    <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {artists.map((artist) => (
        <li
          key={artist.id}
          className="flex items-center gap-3 rounded-xl border border-line/60 bg-surface/50 p-3"
        >
          <span
            aria-hidden="true"
            className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-accent-indigo/30 to-accent-violet/30 text-base font-semibold text-copy"
          >
            {artist.name.slice(0, 1).toUpperCase()}
          </span>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-semibold text-copy">{artist.name}</p>
            <p className="text-xs text-copy-muted">
              {artist.track_count} {artist.track_count === 1 ? 'faixa' : 'faixas'} ·{' '}
              {artist.album_count} {artist.album_count === 1 ? 'álbum' : 'álbuns'}
            </p>
          </div>
          <FavoriteButton kind="artist" entity={artist} label={artist.name} />
        </li>
      ))}
    </ul>
  );
}
