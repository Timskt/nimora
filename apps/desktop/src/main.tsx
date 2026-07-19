import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { PetOverlay } from "./components/PetOverlay";
import { isPetView, petBodyClasses } from "./petView";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("Nimora root element is missing");

const petView = isPetView(window.location.search);
const nativeRuntime = "__TAURI_INTERNALS__" in window;
document.body.classList.add(...petBodyClasses(petView, nativeRuntime));

createRoot(root).render(
  <StrictMode>
    {petView ? <PetOverlay /> : <App />}
  </StrictMode>,
);
