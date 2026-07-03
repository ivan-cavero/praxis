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
      path: '/settings',
      name: 'settings',
      component: () => import('../views/SettingsView.vue'),
    },
  ],
})

export default router
