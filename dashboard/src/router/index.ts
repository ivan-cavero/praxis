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
      path: '/projects/:id/chat',
      name: 'project-chat',
      component: () => import('../views/ProjectChatView.vue'),
    },
    // Settings is now a dialog overlay (not a route).
  ],
})

export default router
