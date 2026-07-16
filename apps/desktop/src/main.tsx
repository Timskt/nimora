import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { PetOverlay } from "./components/PetOverlay";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("AsterPet root element is missing");

export function isPetView(search: string): boolean {
  return new URLSearchParams(search).get("view") === "pet";
}

const petView = isPetView(window.location.search);
if (petView) document.body.classList.add("pet-window");

createRoot(root).render(
  <StrictMode>
    {petView ? <PetOverlay /> : <App />}
  </StrictMode>,
);
