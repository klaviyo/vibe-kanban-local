/**
 * Error type for remote-shape fetch operations.
 * Wraps HTTP / network failures so consumers (banner UI, retry handlers) can
 * inspect status codes uniformly.
 */
export interface SyncError {
  /** HTTP status code if available */
  status?: number;
  /** Error message */
  message: string;
}

/**
 * Result of an optimistic mutation operation.
 * Contains a promise that resolves when the backend confirms the change.
 */
export interface MutationResult {
  /** Promise that resolves when the mutation is confirmed by the backend */
  persisted: Promise<void>;
}

/**
 * Result of an insert operation, including the created row data.
 */
export interface InsertResult<TRow> {
  /** The optimistically created row with generated ID */
  data: TRow;
  /** Promise that resolves with the synced row (including server-generated fields) when confirmed by backend */
  persisted: Promise<TRow>;
}
