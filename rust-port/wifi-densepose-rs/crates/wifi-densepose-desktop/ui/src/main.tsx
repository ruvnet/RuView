import React from "react";
import ReactDOM from "react-dom/client";
import "./design-system.css";
import App from "./App";

// Detect macOS for native titlebar integration
if (navigator.userAgent.includes("Mac")) {
  document.documentElement.classList.add("macos-app");
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
