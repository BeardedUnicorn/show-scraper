import { NavLink, Outlet } from "react-router-dom";
import { useMemo } from "react";
import "./App.css";

const NAVIGATION = [
  { label: "Dashboard", to: "/" },
  { label: "Pending Posts", to: "/pending" },
  { label: "Venues", to: "/venues" },
  { label: "History", to: "/history" },
  { label: "Settings", to: "/settings" },
];

export default function App() {
  const navItems = useMemo(() => NAVIGATION, []);

  return (
    <div className="app-shell">
      <aside className="app-shell__sidebar">
        <div className="app-shell__brand">
          <span className="app-shell__logo">🎵</span>
          <div>
            <div className="app-shell__title">ShowScraper</div>
            <div className="app-shell__subtitle">Tauri v2</div>
          </div>
        </div>
        <nav className="app-shell__nav">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.to === "/"}
              className={({ isActive }) =>
                `app-shell__nav-link${isActive ? " app-shell__nav-link--active" : ""}`
              }
            >
              {item.label}
            </NavLink>
          ))}
        </nav>
      </aside>
      <main className="app-shell__content">
        <Outlet />
      </main>
    </div>
  );
}
