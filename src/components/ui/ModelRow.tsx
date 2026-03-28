type ModelRowProps = {
  name: string;
  sizeLabel: string;
  descriptor: string;
  isInstalled: boolean;
  isActive: boolean;
  isDownloading: boolean;
  progress: number;
  error: string | null;
  onDownload: () => void;
  onCancel: () => void;
  onDelete: () => void;
  onSetActive: () => void;
};

function ModelRow({
  name,
  sizeLabel,
  descriptor,
  isInstalled,
  isActive,
  isDownloading,
  progress,
  error,
  onDownload,
  onCancel,
  onDelete,
  onSetActive,
}: ModelRowProps) {
  const pct = Math.round(progress * 100);

  return (
    <article className="mac-list-row">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-[13px] font-semibold text-[var(--kk-text)]">{name}</h3>
            <span className="text-[11px] text-[var(--kk-text-tertiary)]">{sizeLabel}</span>
            {isActive && <span className="mac-chip mac-chip--success">Active</span>}
          </div>
          <p className="mt-1 text-[12px] text-[var(--kk-text-secondary)]">{descriptor}</p>
        </div>

        <div className="flex items-center gap-1.5">
          {!isInstalled && !isDownloading && (
            <button type="button" className="mac-btn" onClick={onDownload}>
              Download
            </button>
          )}
          {isDownloading && (
            <button type="button" className="mac-btn" onClick={onCancel}>
              Cancel
            </button>
          )}
          {isInstalled && !isActive && (
            <button type="button" className="mac-btn" onClick={onSetActive}>
              Set Active
            </button>
          )}
          {isInstalled && (
            <button type="button" className="mac-btn mac-btn-destructive" onClick={onDelete}>
              Delete
            </button>
          )}
        </div>
      </div>

      {isDownloading && (
        <div className="mt-2.5">
          <div className="mac-progress-track">
            <div className="mac-progress-fill" style={{ width: `${pct}%` }} />
          </div>
          <p className="mt-1 text-[11px] text-[var(--kk-text-tertiary)]">Downloading {pct}%</p>
        </div>
      )}

      {error && <p className="mt-2 text-[11px] text-[var(--kk-danger)]">{error}</p>}
    </article>
  );
}

export default ModelRow;
