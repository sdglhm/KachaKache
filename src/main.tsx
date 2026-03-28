import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import About from "./About";
import Overlay from "./Overlay";
import "./index.css";

const isOverlay = new URLSearchParams(window.location.search).get("overlay") === "1";
const isAbout = new URLSearchParams(window.location.search).get("about") === "1";
const isSetup = new URLSearchParams(window.location.search).get("setup") === "1";

if (isOverlay) {
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isOverlay ? <Overlay /> : isAbout ? <About /> : <App windowMode={isSetup ? "setup" : "main"} />}
  </React.StrictMode>,
);
