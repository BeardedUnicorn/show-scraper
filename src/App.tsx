import { NavLink, Outlet } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
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
  const [sidebarOpen, setSidebarOpen] = useState(() => {
    if (typeof window === "undefined") {
      return true;
    }
    return window.innerWidth >= 1024;
  });
  const [isDesktop, setIsDesktop] = useState(() => {
    if (typeof window === "undefined") {
      return true;
    }
    return window.innerWidth >= 1024;
  });

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    function handleResize() {
      const desktop = window.innerWidth >= 1024;
      setIsDesktop(desktop);
      setSidebarOpen(desktop);
    }

    handleResize();
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  function handleToggleSidebar() {
    if (isDesktop) {
      return;
    }
    setSidebarOpen((prev) => !prev);
  }

  function handleCloseSidebar() {
    if (isDesktop) {
      return;
    }
    setSidebarOpen(false);
  }

  return (
    <div className={`app-shell${sidebarOpen ? " app-shell--sidebar-open" : ""}`}>
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      {!isDesktop && sidebarOpen && (
        <button
          type="button"
          className="app-shell__backdrop"
          aria-label="Close navigation"
          onClick={handleCloseSidebar}
        />
      )}
      <aside className="app-shell__sidebar" aria-label="Primary navigation">
        <div className="app-shell__brand">
          <span className="app-shell__logo" aria-hidden="true">
            🎵
          </span>
          <div>
            <div className="app-shell__title">ShowScraper</div>
            <div className="app-shell__subtitle">Tauri v2</div>
          </div>
        </div>
        <nav id="primary-navigation" className="app-shell__nav">
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
      <div className="app-shell__main">
        <header className="app-shell__header">
          <button
            type="button"
            className="app-shell__menu-button"
            onClick={handleToggleSidebar}
            aria-controls="primary-navigation"
            aria-expanded={sidebarOpen}
          >
            <span className="sr-only">Toggle navigation</span>
            ☰
          </button>
          <div>
            <div className="app-shell__header-title">ShowScraper</div>
            <div className="app-shell__header-subtitle">
              Monitor scraping health, posting status, and upcoming tasks.
            </div>
          </div>
        </header>
        <main id="main-content" className="app-shell__content" tabIndex={-1}>
          <Outlet />
        </main>
      </div>
    </div>
  );
}
