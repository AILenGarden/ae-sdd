import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "../../src/styles.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("React root element was not found.");
}

createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
