import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { hydrateThemeFromBackend } from "./theme";
import App from "./App";
import { CloseGuardProvider } from "./CloseGuard";

void hydrateThemeFromBackend().finally(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <BrowserRouter>
        <CloseGuardProvider>
          <App />
        </CloseGuardProvider>
      </BrowserRouter>
    </React.StrictMode>,
  );
});
