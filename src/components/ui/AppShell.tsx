import type { ReactNode } from "react";
import AmbientBackground from "./AmbientBackground";

type AppShellProps = {
  children: ReactNode;
  variant?: "ambient" | "native";
};

function AppShell({ children, variant = "native" }: AppShellProps) {
  const showAmbient = variant === "ambient";

  return (
    <main className="mac-app-shell">
      {showAmbient && <AmbientBackground />}
      <div className="mac-window">{children}</div>
    </main>
  );
}

export default AppShell;
