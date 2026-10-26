import type { FavoriteKind } from '@zuno/shared-types';
import { useFavoritesOptional } from '../favorites/FavoritesProvider';
import type { FavoriteEntity } from '../favorites/FavoritesProvider';
import { HeartIcon } from './Icons';

interface FavoriteButtonProps {
  kind: FavoriteKind;
  entity: FavoriteEntity;
  label: string;
  className?: string;
}

export function FavoriteButton({ kind, entity, label, className }: FavoriteButtonProps) {
  const favorites = useFavoritesOptional();

  if (!favorites) {
    return null;
  }

  const active = favorites.isFavorite(kind, entity.id);
  const action = active ? 'Remover' : 'Adicionar';
  const accessibleLabel = `${action} ${label} ${active ? 'dos' : 'aos'} favoritos`;

  return (
    <button
      type="button"
      aria-pressed={active}
      aria-label={accessibleLabel}
      title={active ? 'Remover dos favoritos' : 'Adicionar aos favoritos'}
      onClick={(event) => {
        event.stopPropagation();
        void favorites.toggleFavorite(kind, entity);
      }}
      className={`inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full border transition-colors ${
        active
          ? 'border-accent-magenta/60 bg-accent-magenta/10 text-accent-magenta'
          : 'border-transparent text-copy-muted hover:border-line hover:text-accent-magenta'
      } ${className ?? ''}`}
    >
      <HeartIcon filled={active} className="h-4 w-4" />
    </button>
  );
}
