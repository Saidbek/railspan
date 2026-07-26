import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      name: 'endpoints',
      component: () => import('@/views/EndpointsView.vue'),
      meta: { title: 'Endpoints' },
    },
    {
      path: '/jobs',
      name: 'jobs',
      component: () => import('@/views/JobsView.vue'),
      meta: { title: 'Jobs' },
    },
    {
      path: '/n-plus-one',
      name: 'n1',
      component: () => import('@/views/N1View.vue'),
      meta: { title: 'N+1' },
    },
    {
      path: '/deploys',
      name: 'deploys',
      component: () => import('@/views/DeploysView.vue'),
      meta: { title: 'Deploys' },
    },
    {
      // resource is encodeURIComponent'd (handles `#` in Controller#action)
      path: '/resources/:resource',
      name: 'resource',
      component: () => import('@/views/ResourceView.vue'),
      meta: { title: 'Resource' },
    },
    {
      path: '/traces/:traceId',
      name: 'trace',
      component: () => import('@/views/TraceView.vue'),
      meta: { title: 'Trace' },
    },
    {
      path: '/:pathMatch(.*)*',
      redirect: '/',
    },
  ],
  scrollBehavior() {
    return { top: 0 }
  },
})

router.afterEach((to) => {
  const title = (to.meta.title as string | undefined) || 'Railspan'
  document.title = `${title} · Railspan`
})

export default router
