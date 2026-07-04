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
    // Settings is now a dialog overlay (not a route).
    // If you need a direct URL, capture it and open the dialog programmatically.
  ],
})

export default router
