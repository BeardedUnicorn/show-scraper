import { createHashRouter } from "react-router-dom";
import App from "../App";
import Dashboard from "./routes/Dashboard";
import Venues from "./routes/Venues";
import Settings from "./routes/Settings";
import History from "./routes/History";
import PendingPosts from "./routes/PendingPosts";

export const router = createHashRouter([
  {
    path: "/",
    element: <App />,
    children: [
      { index: true, element: <Dashboard /> },
      { path: "pending", element: <PendingPosts /> },
      { path: "venues", element: <Venues /> },
      { path: "history", element: <History /> },
      { path: "settings", element: <Settings /> },
    ],
  },
]);
