import { getName, getVersion } from "@tauri-apps/api/app";
import { useEffect, useMemo, useState } from "react";
import appIcon from "./assets/kachakache-icon.svg";

type AboutSection = "overview" | "credits" | "licenses";

const credits = [
  {
    name: "Tauri + Rust",
    note: "Desktop shell, native integrations, and local orchestration.",
    meta: "Core runtime",
  },
  {
    name: "React + TypeScript",
    note: "Application interface and local state management.",
    meta: "Frontend",
  },
  {
    name: "whisper.cpp + whisper-rs",
    note: "On-device speech recognition and local model execution.",
    meta: "Speech engine",
  },
  {
    name: "CPAL + macOS accessibility APIs",
    note: "Microphone capture, hotkeys, and focused-app text insertion.",
    meta: "Input pipeline",
  },
];

const licenses = [
  {
    name: "KachaKache",
    note: "This build bundles local speech runtime components for on-device transcription.",
    meta: "Project distribution",
  },
  { name: "Tauri", note: "Apache-2.0 or MIT", meta: "Open source" },
  { name: "React", note: "MIT", meta: "Open source" },
  { name: "whisper.cpp", note: "MIT", meta: "Open source" },
  { name: "whisper-rs", note: "MIT", meta: "Open source" },
];

function About() {
  const [appName, setAppName] = useState("KachaKache");
  const [version, setVersion] = useState("0.1.0");
  const [section, setSection] = useState<AboutSection>("overview");

  useEffect(() => {
    void getName().then(setAppName).catch(() => undefined);
    void getVersion().then(setVersion).catch(() => undefined);
  }, []);

  const panelContent = useMemo(() => {
    if (section === "credits") {
      return (
        <>
          <p className="about-panel-title">Credits</p>
          <div className="about-panel-list">
            {credits.map((entry) => (
              <div key={entry.name} className="about-panel-row">
                <div>
                  <p className="about-panel-name">{entry.name}</p>
                  <p className="about-panel-note">{entry.note}</p>
                </div>
                <span className="about-panel-meta">{entry.meta}</span>
              </div>
            ))}
          </div>
        </>
      );
    }

    if (section === "licenses") {
      return (
        <>
          <p className="about-panel-title">Licenses</p>
          <div className="about-panel-list">
            {licenses.map((entry) => (
              <div key={entry.name} className="about-panel-row">
                <div>
                  <p className="about-panel-name">{entry.name}</p>
                  <p className="about-panel-note">{entry.note}</p>
                </div>
                <span className="about-panel-meta">{entry.meta}</span>
              </div>
            ))}
          </div>
        </>
      );
    }

    return (
      <>
        <p className="about-panel-title">Overview</p>
        <div className="about-panel-copy">
          <p>
            KachaKache is a local-first dictation app for macOS. Audio stays on this Mac, speech is
            transcribed using downloadable local models, and final text is inserted into the active
            app without relying on cloud processing.
          </p>
          <p className="mt-3">
            This MVP is designed as a lightweight utility that lives comfortably in the menu bar,
            with a compact settings window and a focused overlay while dictation is active.
          </p>
        </div>
      </>
    );
  }, [section]);

  return (
    <main className="about-window">
      <section className="about-card">
        <div className="about-header">
          <img src={appIcon} alt="" className="about-icon" />
          <h1 className="about-title">{appName}</h1>
          <p className="about-version">Version {version}</p>
          <p className="about-description">Private, local dictation for macOS.</p>
          <p className="about-meta">Runs on-device with Rust, Tauri, and Whisper-compatible models.</p>
        </div>

        <div className="about-segmented" role="tablist" aria-label="About sections">
          {[
            ["overview", "Overview"],
            ["credits", "Credits"],
            ["licenses", "Licenses"],
          ].map(([id, label]) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={section === id}
              className={`about-segmented-btn ${section === id ? "is-active" : ""}`}
              onClick={() => setSection(id as AboutSection)}
            >
              {label}
            </button>
          ))}
        </div>

        <section className="about-panel" role="tabpanel">
          {panelContent}
        </section>

        <div className="about-actions">
          <span className="about-footnote">© 2026 KachaKache. Built for local-only dictation.</span>
        </div>
      </section>
    </main>
  );
}

export default About;
