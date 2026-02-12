import { createRouter, createWebHistory } from "vue-router";
import DashboardView from "../views/Dashboard.vue";
import TasksView from "../views/Tasks.vue";
import ConflictsView from "../views/Conflicts.vue";
import LogsView from "../views/Logs.vue";
import SettingsView from "../views/Settings.vue";
import AboutView from "../views/About.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      name: "dashboard",
      component: DashboardView,
      meta: { title: "Overview", subtitle: "Sync health and recent activity" }
    },
    {
      path: "/tasks",
      name: "tasks",
      component: TasksView,
      meta: { title: "Tasks", subtitle: "Task list and runtime status" }
    },
    {
      path: "/conflicts",
      name: "conflicts",
      component: ConflictsView,
      meta: { title: "Conflicts", subtitle: "Conflict tracking with dual-retention" }
    },
    {
      path: "/logs",
      name: "logs",
      component: LogsView,
      meta: { title: "Activity Logs", subtitle: "Auditable and traceable sync records" }
    },
    {
      path: "/settings",
      name: "settings",
      component: SettingsView,
      meta: { title: "Settings", subtitle: "Performance, network and preferences" }
    },
    {
      path: "/about",
      name: "about",
      component: AboutView,
      meta: { title: "About", subtitle: "Diagnostics and version info" }
    }
  ]
});

export default router;
