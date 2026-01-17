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
      meta: { title: "概览", subtitle: "同步健康度与最近活动" }
    },
    {
      path: "/tasks",
      name: "tasks",
      component: TasksView,
      meta: { title: "同步任务", subtitle: "任务列表与运行状态" }
    },
    {
      path: "/conflicts",
      name: "conflicts",
      component: ConflictsView,
      meta: { title: "冲突中心", subtitle: "双保留策略下的冲突追踪" }
    },
    {
      path: "/logs",
      name: "logs",
      component: LogsView,
      meta: { title: "活动日志", subtitle: "可审计、可追溯的同步记录" }
    },
    {
      path: "/settings",
      name: "settings",
      component: SettingsView,
      meta: { title: "设置", subtitle: "性能、网络与偏好" }
    },
    {
      path: "/about",
      name: "about",
      component: AboutView,
      meta: { title: "关于", subtitle: "诊断与版本信息" }
    }
  ]
});

export default router;
