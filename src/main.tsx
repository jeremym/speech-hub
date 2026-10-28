import React from "react";
import ReactDOM from "react-dom/client";
import "@overtone-foundry/app-kit/kit.css";
import "./App.css";
import { applyLook } from "@overtone-foundry/app-kit";
import App from "./App";
import { initialLook, LOOK_KEY } from "./look";

applyLook(initialLook(), LOOK_KEY);

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
