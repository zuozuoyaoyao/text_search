import { createApp } from 'vue'
import ArcoVue from '@arco-design/web-vue'
import '@arco-design/web-vue/dist/arco.css'
import { connect } from './rpc'
import App from './App.vue'
import SettingsApp from './SettingsApp.vue'

connect().catch((e) => console.error('ws connect failed', e))

function applyTheme() {
  const dark = window.matchMedia('(prefers-color-scheme: dark)').matches
  document.body.setAttribute('arco-theme', dark ? 'dark' : 'light')
}
applyTheme()
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme)

const urlParams = new URLSearchParams(window.location.search)
const page = urlParams.get('page')

const app = createApp(page === 'settings' ? SettingsApp : App)
app.use(ArcoVue)
app.mount('#app')
