import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'dashboard',
      component: () => import('../views/DashboardView.vue'),
    },
    {
      path: '/pipeline',
      name: 'pipeline',
      component: () => import('../views/PipelineView.vue'),
    },
    {
      path: '/sessions',
      name: 'sessions',
      component: () => import('../views/SessionsView.vue'),
    },
    {
      path: '/sessions/:id',
      name: 'session-detail',
      component: () => import('../views/SessionView.vue'),
    },
    {
      path: '/cost-analysis',
      name: 'cost-analysis',
      component: () => import('../views/CostAnalysisView.vue'),
    },
    {
      path: '/agents',
      name: 'agents',
      component: () => import('../views/AgentsView.vue'),
    },
    {
      path: '/projects/:id/chat',
      name: 'project-chat',
      component: () => import('../views/ProjectChatView.vue'),
    },
    {
      path: '/agent-debug',
      name: 'agent-debug',
      component: () => import('../views/AgentDebugView.vue'),
    },
    // 404 catch-all
    {
      path: '/memory',
      name: 'memory',
      component: () => import('../views/MemoryBrowserView.vue'),
    },
    {
      path: '/logs',
      name: 'logs',
      component: () => import('../views/LogsView.vue'),
    },
    {
      path: '/:pathMatch(.*)*',
      name: 'not-found',
      component: () => import('../views/NotFoundView.vue'),
    },
  ],
})

export default router
