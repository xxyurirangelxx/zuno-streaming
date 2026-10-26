/**
 * Contratos compartilhados entre servidor, web e futuros clientes nativos.
 * Mantidos em sincronia manual com `docs/api/openapi.yaml`.
 */

export type DatabaseHealthStatus = 'ok' | 'error';

export type ServiceHealthStatus = 'ok' | 'degraded';

export interface DatabaseHealth {
  status: DatabaseHealthStatus;
}

export interface HealthResponse {
  status: ServiceHealthStatus;
  service: string;
  version: string;
  uptime_seconds: number;
  database: DatabaseHealth;
}

export interface ApiErrorBody {
  code: string;
  message: string;
}

// --- Autenticação ---------------------------------------------------------

export type UserRole = 'admin' | 'moderator' | 'user';

export interface User {
  id: string;
  username: string;
  role: UserRole;
}

export interface SessionResponse {
  user: User;
}

export interface SetupStatusResponse {
  required: boolean;
}

export interface LoginRequest {
  username: string;
  password: string;
}

export interface SetupClaimRequest {
  code: string;
  username: string;
  password: string;
}

export interface RedeemInvitationRequest {
  token: string;
  username: string;
  password: string;
}

export type InvitationRole = 'user' | 'moderator';

export interface CreateInvitationRequest {
  role?: InvitationRole;
  expires_in_days?: number;
}

export interface CreatedInvitation {
  id: string;
  /** Aparece apenas na criação. */
  token: string;
  role: InvitationRole;
  expires_in_days: number;
}

export interface InvitationItem {
  id: string;
  role: InvitationRole;
  expires_at: string;
  redeemed_at: string | null;
  revoked_at: string | null;
}

// --- Biblioteca -----------------------------------------------------------

export interface ArtistRef {
  id: string;
  name: string;
}

export interface AlbumRef {
  id: string;
  title: string;
}

export interface TrackSummary {
  id: string;
  title: string;
  duration_ms: number | null;
  track_no: number | null;
  disc_no: number | null;
  album: AlbumRef | null;
  artists: ArtistRef[];
  artwork_id: string | null;
  available: boolean;
}

export interface AudioFileInfo {
  id: string;
  codec: string;
  container: string;
  bitrate: number | null;
  sample_rate: number | null;
  channels: number | null;
  size_bytes: number;
  available: boolean;
}

export interface TrackDetail extends TrackSummary {
  audio: AudioFileInfo | null;
}

export interface AlbumSummary {
  id: string;
  title: string;
  artist: ArtistRef | null;
  release_year: number | null;
  artwork_id: string | null;
  track_count: number;
}

export interface ArtistSummary {
  id: string;
  name: string;
  track_count: number;
  album_count: number;
}

export interface TrackListResponse {
  items: TrackSummary[];
  next_cursor: string | null;
}

export interface AlbumListResponse {
  items: AlbumSummary[];
  next_cursor: string | null;
}

export interface ArtistListResponse {
  items: ArtistSummary[];
  next_cursor: string | null;
}

export interface SearchResponse {
  query: string;
  tracks: TrackSummary[];
  albums: AlbumSummary[];
  artists: ArtistSummary[];
}

export interface ListTracksParams {
  limit?: number;
  cursor?: string;
  album_id?: string;
  artist_id?: string;
  q?: string;
}

export interface ListPageParams {
  limit?: number;
  cursor?: string;
}

/** Variantes aceitas por `GET /api/v1/artworks/{id}?size=`. */
export type ArtworkSize = 'full' | 'thumb' | 'card' | number;
