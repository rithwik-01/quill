import * as React from "react";
import * as ReactDOM from "react-dom/client";
import { Popup } from "./Popup";
import { Toaster } from "sonner";
import "../index.css";

ReactDOM.createRoot(document.getElementById("popup-root")!).render(
  <React.StrictMode>
    <Popup />
    <Toaster richColors />
  </React.StrictMode>,
);
