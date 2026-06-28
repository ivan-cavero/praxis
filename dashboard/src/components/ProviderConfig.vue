<script setup lang="ts">
import { ref } from 'vue'

// Provider configuration
const providers = ref([
  { id: 'openai', name: 'OpenAI', base_url: 'https://api.openai.com/v1', models: ['gpt-5', 'gpt-4o', 'gpt-4o-mini'], status: 'configured' },
  { id: 'anthropic', name: 'Anthropic', base_url: 'https://api.anthropic.com', models: ['claude-4-opus', 'claude-4-haiku'], status: 'configured' },
  { id: 'gemini', name: 'Google Gemini', base_url: 'https://generativelanguage.googleapis.com', models: ['gemini-2.5-pro'], status: 'configured' },
  { id: 'nan', name: 'Nan (Custom)', base_url: 'https://api.nan.builders/v1', models: ['deepseek-v4-flash', 'mimo-v2.5', 'qwen3.6', 'gemma4', 'qwen3-embedding'], status: 'configured' },
])

const newProvider = ref({
  name: '',
  base_url: '',
  api_key: '',
})

const showAddForm = ref(false)

function addProvider() {
  if (newProvider.value.name && newProvider.value.base_url) {
    providers.value.push({
      id: newProvider.value.name.toLowerCase(),
      name: newProvider.value.name,
      base_url: newProvider.value.base_url,
      models: [],
      status: 'configured',
    })
    newProvider.value = { name: '', base_url: '', api_key: '' }
    showAddForm.value = false
  }
}

function removeProvider(id: string) {
  providers.value = providers.value.filter(p => p.id !== id)
}

const modelInfo: Record<string, { context: string, capabilities: string[] }> = {
  'gpt-5': { context: '128K', capabilities: ['Tool calling', 'Streaming'] },
  'gpt-4o': { context: '128K', capabilities: ['Tool calling', 'Streaming', 'Vision'] },
  'claude-4-opus': { context: '200K', capabilities: ['Tool calling', 'Streaming', 'Vision'] },
  'claude-4-haiku': { context: '200K', capabilities: ['Tool calling', 'Streaming'] },
  'gemini-2.5-pro': { context: '1M', capabilities: ['Tool calling', 'Streaming', 'Vision'] },
  'deepseek-v4-flash': { context: '1M', capabilities: ['Tool calling', 'Reasoning', 'Streaming'] },
  'mimo-v2.5': { context: '1M', capabilities: ['Tool calling', 'Reasoning', 'Vision', 'Audio'] },
  'qwen3.6': { context: '256K', capabilities: ['Tool calling', 'Reasoning', 'Vision'] },
  'gemma4': { context: '256K', capabilities: ['Tool calling', 'Vision'] },
  'qwen3-embedding': { context: '8K', capabilities: ['Embeddings'] },
}
</script>

<template>
  <div class="space-y-6">

    <!-- Providers List -->
    <section>
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-xs font-semibold text-zinc-500 tracking-widest">PROVIDERS</h2>
        <button @click="showAddForm = !showAddForm"
          class="text-xs px-3 py-1 rounded bg-zinc-800 text-zinc-300 hover:bg-zinc-700 transition-colors">
          + Add Provider
        </button>
      </div>

      <div class="space-y-2">
        <div v-for="p in providers" :key="p.id"
          class="bg-zinc-900 border border-zinc-800 rounded-lg p-4 hover:border-zinc-700 transition-colors">
          <div class="flex items-center justify-between mb-2">
            <div class="flex items-center gap-2">
              <span class="w-2 h-2 rounded-full" :class="p.status === 'configured' ? 'bg-emerald-500' : 'bg-zinc-500'"></span>
              <span class="font-medium text-sm">{{ p.name }}</span>
            </div>
            <button @click="removeProvider(p.id)" class="text-xs text-zinc-500 hover:text-red-400">✕</button>
          </div>
          <div class="text-xs text-zinc-500 mb-2 font-mono">{{ p.base_url }}</div>
          <div class="flex flex-wrap gap-1">
            <span v-for="m in p.models" :key="m"
              class="text-xs px-2 py-0.5 rounded bg-zinc-800 text-zinc-400">
              {{ m }}
            </span>
          </div>
        </div>
      </div>

      <!-- Add Provider Form -->
      <div v-if="showAddForm" class="mt-3 bg-zinc-900 border border-zinc-700 rounded-lg p-4 space-y-3">
        <div class="text-sm font-medium text-zinc-300">Add Custom Provider</div>
        <input v-model="newProvider.name" placeholder="Provider name"
          class="w-full bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-zinc-500" />
        <input v-model="newProvider.base_url" placeholder="Base URL (https://api.example.com/v1)"
          class="w-full bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-zinc-500" />
        <input v-model="newProvider.api_key" placeholder="API Key" type="password"
          class="w-full bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-zinc-500" />
        <div class="flex gap-2">
          <button @click="addProvider"
            class="px-4 py-1.5 rounded bg-emerald-600 text-white text-sm hover:bg-emerald-500">Save</button>
          <button @click="showAddForm = false"
            class="px-4 py-1.5 rounded bg-zinc-800 text-zinc-400 text-sm hover:bg-zinc-700">Cancel</button>
        </div>
      </div>
    </section>

    <!-- Model Reference -->
    <section>
      <h2 class="text-xs font-semibold text-zinc-500 tracking-widest mb-3">MODEL REFERENCE</h2>
      <div class="bg-zinc-900 border border-zinc-800 rounded-lg overflow-hidden">
        <table class="w-full text-sm">
          <thead>
            <tr class="text-xs text-zinc-500 border-b border-zinc-800">
              <th class="text-left px-4 py-2 font-medium">MODEL</th>
              <th class="text-left px-4 py-2 font-medium">CONTEXT</th>
              <th class="text-left px-4 py-2 font-medium">CAPABILITIES</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(info, model) in modelInfo" :key="model" class="border-t border-zinc-800/50 hover:bg-zinc-800/30">
              <td class="px-4 py-2 font-medium font-mono text-xs">{{ model }}</td>
              <td class="px-4 py-2 text-zinc-400 text-xs">{{ info.context }}</td>
              <td class="px-4 py-2">
                <div class="flex flex-wrap gap-1">
                  <span v-for="cap in info.capabilities" :key="cap"
                    class="text-xs px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-400">
                    {{ cap }}
                  </span>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </div>
</template>