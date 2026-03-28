import { useMemo, useState, type MouseEventHandler } from "react";
import type {
  DictationStatus,
  DownloadProgressEvent,
  InstalledModel,
  PermissionKind,
  PermissionsStatus,
  RecommendedModel,
} from "../../types";
import BrandPill from "./BrandPill";

type PracticeResult = {
  transcript: string;
  score: number;
};

type SetupSplashProps = {
  permissions: PermissionsStatus;
  recommendedModels: RecommendedModel[];
  selectedModelId: string | null;
  selectedInstalledModel: InstalledModel | null;
  selectedModelProgress?: DownloadProgressEvent;
  practicePhrases: string[];
  selectedPhraseIndex: number;
  practiceResult: PracticeResult | null;
  status: DictationStatus;
  onSelectModel: (modelId: string) => void;
  onSelectPhrase: (index: number) => void;
  onRequestPermission: (kind: PermissionKind) => void;
  onOpenPermissionSettings: (kind: PermissionKind) => void;
  onDownloadModel: (modelId: string) => void;
  onCancelModel: (modelId: string) => void;
  onSetActiveModel: (modelId: string) => void;
  onTogglePractice: () => void;
  onFinish: () => void;
  onSkip: () => void;
  onToolbarMouseDown?: MouseEventHandler<HTMLElement>;
};

function SetupSplash({
  permissions,
  recommendedModels,
  selectedModelId,
  selectedInstalledModel,
  selectedModelProgress,
  practicePhrases,
  selectedPhraseIndex,
  practiceResult,
  status,
  onSelectModel,
  onSelectPhrase,
  onRequestPermission,
  onOpenPermissionSettings,
  onDownloadModel,
  onCancelModel,
  onSetActiveModel,
  onTogglePractice,
  onFinish,
  onSkip,
  onToolbarMouseDown,
}: SetupSplashProps) {
  const [currentStep, setCurrentStep] = useState(0);

  const selectedModel =
    recommendedModels.find((model) => model.id === selectedModelId) ?? recommendedModels[0] ?? null;
  const microphoneReady = permissions.microphoneGranted;
  const accessibilityReady = permissions.accessibilityGranted;
  const permissionsReady = microphoneReady && accessibilityReady;
  const modelReady = Boolean(selectedInstalledModel?.isActive);
  const isDownloading = Boolean(selectedModelProgress && !selectedModelProgress.done);
  const practiceComplete = Boolean(practiceResult);
  const canContinue =
    (currentStep === 0 && permissionsReady) ||
    (currentStep === 1 && modelReady) ||
    (currentStep === 2 && practiceComplete);

  const stepMeta = useMemo(
    () => [
      {
        index: 1,
        title: "Permissions",
        description: "Allow access to your microphone and accessibility features.",
      },
      {
        index: 2,
        title: "Model",
        description: "Choose a local speech model to download and use.",
      },
      {
        index: 3,
        title: "Practice",
        description: "Read a short phrase and check how closely it matches.",
      },
    ],
    [],
  );

  const renderPermissionsStep = () => (
    <section className="mac-setup-step">
      <div className="mac-setup-step-head">
        <div>
          <p className="mac-kicker">Step 1</p>
          <h2 className="mac-page-title mac-page-title--compact">Allow access</h2>
          <p className="mac-page-subtitle">
            KachaKache stays local, but it still needs permission to listen and type.
          </p>
        </div>
        <span className={`mac-badge ${permissionsReady ? "mac-badge--success" : "mac-badge--neutral"}`}>
          {permissionsReady ? "Ready" : "Pending"}
        </span>
      </div>

      <div className="mac-setup-row">
        <div>
          <p className="mac-setting-label">Microphone</p>
          <p className="mac-setting-description">Capture audio locally on this Mac.</p>
        </div>
        <div className="mac-setup-actions">
          <span className={`mac-badge ${microphoneReady ? "mac-badge--success" : "mac-badge--error"}`}>
            {microphoneReady ? "Allowed" : "Required"}
          </span>
          {!microphoneReady && (
            <>
              <button type="button" className="mac-btn" onClick={() => onRequestPermission("microphone")}>
                Request
              </button>
              <button type="button" className="mac-btn" onClick={() => onOpenPermissionSettings("microphone")}>
                Open Settings
              </button>
            </>
          )}
        </div>
      </div>

      <div className="mac-setup-row">
        <div>
          <p className="mac-setting-label">Accessibility</p>
          <p className="mac-setting-description">Insert dictated text into the active app.</p>
        </div>
        <div className="mac-setup-actions">
          <span className={`mac-badge ${accessibilityReady ? "mac-badge--success" : "mac-badge--error"}`}>
            {accessibilityReady ? "Allowed" : "Required"}
          </span>
          {!accessibilityReady && (
            <>
              <button
                type="button"
                className="mac-btn"
                onClick={() => onRequestPermission("accessibility")}
              >
                Request
              </button>
              <button
                type="button"
                className="mac-btn"
                onClick={() => onOpenPermissionSettings("accessibility")}
              >
                Open Settings
              </button>
            </>
          )}
        </div>
      </div>
    </section>
  );

  const renderModelStep = () => (
    <section className="mac-setup-step">
      <div className="mac-setup-step-head">
        <div>
          <p className="mac-kicker">Step 2</p>
          <h2 className="mac-page-title mac-page-title--compact">Pick a model</h2>
          <p className="mac-page-subtitle">
            Start with a local model that balances speed and accuracy for this Mac.
          </p>
        </div>
        <span className={`mac-badge ${modelReady ? "mac-badge--success" : isDownloading ? "mac-badge--processing" : "mac-badge--neutral"}`}>
          {modelReady ? "Active" : isDownloading ? "Downloading" : "Choose one"}
        </span>
      </div>

      <div className="mac-setup-model-list">
        {recommendedModels.map((model) => {
          const installed = model.id === selectedInstalledModel?.id;
          const selected = model.id === selectedModelId;
          return (
            <button
              key={model.id}
              type="button"
              className={`mac-setup-model-row ${selected ? "is-selected" : ""}`}
              onClick={() => onSelectModel(model.id)}
            >
              <div className="min-w-0 flex-1 text-left">
                <p className="mac-setting-label">{model.displayName}</p>
                <p className="mac-setting-description">
                  {model.speedNote} • {model.qualityNote} • {model.sizeMb} MB
                </p>
              </div>
              <span className={`mac-badge ${installed && selectedInstalledModel?.isActive ? "mac-badge--success" : "mac-badge--neutral"}`}>
                {installed ? (selectedInstalledModel?.isActive ? "Active" : "Installed") : "Available"}
              </span>
            </button>
          );
        })}
      </div>

      {selectedModel && (
        <div className="mac-setup-row mac-setup-row--stacked">
          <div>
            <p className="mac-setting-label">{selectedModel.displayName}</p>
            <p className="mac-setting-description">
              {selectedModel.speedNote} • {selectedModel.qualityNote}
            </p>
          </div>
          <div className="mac-setup-actions">
            {selectedInstalledModel ? (
              selectedInstalledModel.isActive ? (
                <span className="mac-badge mac-badge--success">Ready</span>
              ) : (
                <button type="button" className="mac-btn mac-btn-primary" onClick={() => onSetActiveModel(selectedModel.id)}>
                  Use This Model
                </button>
              )
            ) : isDownloading ? (
              <button type="button" className="mac-btn" onClick={() => onCancelModel(selectedModel.id)}>
                Cancel Download
              </button>
            ) : (
              <button type="button" className="mac-btn mac-btn-primary" onClick={() => onDownloadModel(selectedModel.id)}>
                Download Model
              </button>
            )}
          </div>
        </div>
      )}

      {isDownloading && (
        <div className="mac-setup-progress">
          <div className="mac-progress-track">
            <div
              className="mac-progress-fill"
              style={{ width: `${Math.round((selectedModelProgress?.progress ?? 0) * 100)}%` }}
            />
          </div>
          <p className="mac-setting-description">
            Downloading {Math.round((selectedModelProgress?.progress ?? 0) * 100)}%
          </p>
        </div>
      )}
    </section>
  );

  const selectedPhrase = practicePhrases[selectedPhraseIndex] ?? practicePhrases[0] ?? "";
  const scoreLabel = practiceResult ? `${Math.round(practiceResult.score * 100)}% match` : null;

  const renderPracticeStep = () => (
    <section className="mac-setup-step">
      <div className="mac-setup-step-head">
        <div>
          <p className="mac-kicker">Step 3</p>
          <h2 className="mac-page-title mac-page-title--compact">Try a phrase</h2>
          <p className="mac-page-subtitle">
            Read one phrase out loud. We’ll compare the result and show how close it was.
          </p>
        </div>
        <span className={`mac-badge ${practiceComplete ? "mac-badge--success" : status.phase === "listening" ? "mac-badge--processing" : "mac-badge--neutral"}`}>
          {practiceComplete ? scoreLabel : status.phase === "listening" ? "Listening" : "Ready"}
        </span>
      </div>

      <div className="mac-setup-phrase-list">
        {practicePhrases.map((phrase, index) => (
          <button
            key={phrase}
            type="button"
            className={`mac-setup-phrase ${index === selectedPhraseIndex ? "is-selected" : ""}`}
            onClick={() => onSelectPhrase(index)}
          >
            “{phrase}”
          </button>
        ))}
      </div>

      <div className="mac-setup-practice-card">
        <p className="mac-setup-practice-label">Target phrase</p>
        <p className="mac-setup-practice-text">“{selectedPhrase}”</p>
        {practiceResult ? (
          <>
            <div className="mac-setup-practice-score">
              <span className="mac-impact-value">{scoreLabel}</span>
              <span className={`mac-badge ${practiceResult.score >= 0.72 ? "mac-badge--success" : "mac-badge--neutral"}`}>
                {practiceResult.score >= 0.72 ? "Great match" : "Good enough to continue"}
              </span>
            </div>
            <p className="mac-setup-practice-label">What KachaKache heard</p>
            <p className="mac-setup-practice-heard">“{practiceResult.transcript}”</p>
          </>
        ) : (
          <p className="mac-setting-description">
            Start a short dictation to see how closely the transcription matches.
          </p>
        )}
      </div>

      <div className="mac-setup-row mac-setup-row--stacked">
        <div>
          <p className="mac-setting-label">Practice Dictation</p>
          <p className="mac-setting-description">
            {status.phase === "processing"
              ? "Checking your phrase..."
              : "You can repeat the test after changing the phrase or model."}
          </p>
        </div>
        <div className="mac-setup-actions">
          <button
            type="button"
            className={`mac-btn ${status.phase === "listening" ? "" : "mac-btn-primary"}`}
            onClick={onTogglePractice}
            disabled={!permissionsReady || !modelReady || status.phase === "processing"}
          >
            {status.phase === "listening" ? "Stop Practice" : "Start Practice"}
          </button>
        </div>
      </div>
    </section>
  );

  return (
    <div className="mac-setup-root">
      <header className="mac-setup-toolbar" data-tauri-drag-region onMouseDown={onToolbarMouseDown}>
        <BrandPill />
        <button type="button" className="mac-btn" onClick={onSkip}>
          Not Now
        </button>
      </header>

      <div className="mac-setup-shell">
        <section className="mac-setup-card">
          <div className="mac-setup-header">
            <p className="mac-kicker">Welcome</p>
            <h1 className="mac-page-title mac-page-title--flush">Set up KachaKache</h1>
            <p className="mac-page-subtitle">
              A short three-step setup to get local dictation working on this Mac.
            </p>
          </div>

          <div className="mac-setup-stepper" role="tablist" aria-label="Setup steps">
            {stepMeta.map((step, index) => (
              <button
                key={step.index}
                type="button"
                className={`mac-setup-stepper-item ${index === currentStep ? "is-active" : index < currentStep ? "is-complete" : ""}`}
                onClick={() => {
                  if (index <= currentStep || (index === currentStep + 1 && canContinue)) {
                    setCurrentStep(index);
                  }
                }}
              >
                <span className="mac-setup-stepper-count">{step.index}</span>
                <span className="mac-setup-stepper-copy">
                  <span className="mac-setup-stepper-title">{step.title}</span>
                  <span className="mac-setup-stepper-note">{step.description}</span>
                </span>
              </button>
            ))}
          </div>

          <div className="mac-setup-stage">
            {currentStep === 0 && renderPermissionsStep()}
            {currentStep === 1 && renderModelStep()}
            {currentStep === 2 && renderPracticeStep()}
          </div>

          <div className="mac-setup-footer">
            <p className="about-footnote">Everything in setup runs locally on your Mac.</p>
            <div className="mac-setup-actions">
              {currentStep > 0 && (
                <button type="button" className="mac-btn" onClick={() => setCurrentStep((prev) => prev - 1)}>
                  Back
                </button>
              )}
              {currentStep < 2 ? (
                <button
                  type="button"
                  className="mac-btn mac-btn-primary"
                  onClick={() => setCurrentStep((prev) => prev + 1)}
                  disabled={!canContinue}
                >
                  Continue
                </button>
              ) : (
                <button
                  type="button"
                  className="mac-btn mac-btn-primary"
                  onClick={onFinish}
                  disabled={!practiceComplete}
                >
                  Finish Setup
                </button>
              )}
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

export default SetupSplash;
