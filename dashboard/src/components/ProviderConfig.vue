<script setup lang="ts">
import { ref } from 'vue'

// Dynamic provider list (user-configured)
const providers = ref<Array<{
  id: string
  name: string
  base_url: string
  models: string[]
}>>([])

const newProvider = ref({ name: '', base_url: '', api_key: '' })
const showAddForm = ref(false)
const selectedProvider = ref<string | null>(null)

function addProvider() {
  if (newProvider.value.name && newProvider.value.base_url) {
    providers.value.push({
      id: Date.now().toString(),
      name: newProvider.value.name,
      base_url: newProvider.value.base_url,
      models: [],
    })
    newProvider.value = { name: '', base_url: '', api_key: '' }
    showAddForm.value = false
  }
}

function removeProvider(id: string) {
  providers.value = providers.value.filter(p => p.id !== id)
}

function toggleProvider(id: string) {
  selectedProvider.value = selectedProvider.value === id ? null : id
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
          {{ showAddForm ? '✕ Cancel' : '+ Add Provider' }}
        </button>
      </div>

      <div class="space-y-2">
        <div v-for="p in providers" :key="p.id"
          class="bg-zinc-900 border border-zinc-800 rounded-lg p-4 hover:border-zinc-700 transition-colors cursor-pointer"
          @click="toggleProvider(p.id)">
          <div class="flex items-center justify-between mb-2">
            <div class="flex items-center gap-2">
              <span class="w-2 h-2 rounded-full bg-emerald-500"></span>
              <span class="font-medium text-sm">{{ p.name }}</span>
            </div>
            <button @click.stop="removeProvider(p.id)" class="text-xs text-zinc-500 hover:text-red-400">✕</button>
          </div>
          <div class="text-xs text-zinc-500 mb-2 font-mono">{{ p.base_url }}</div>
          <div v-if="p.models.length" class="flex flex-wrap gap-1">
            <span v-for="m in p.models" :key="m"
              class="text-xs px-2 py-0.5 rounded bg-zinc-800 text-zinc-400">
              {{ m }}
            </span>
          </div>
          <div v-else class="text-xs text-zinc-600 italic">No models configured</div>
        </div>

        <div v-if="providers.length === 0" class="text-center py-8 text-zinc-500">
          <div class="text-3xl mb-2">⬡</div>
          <div class="text-sm">No providers configured</div>
          <div class="text-xs mt-1">Click "Add Provider" to get started</div>
        </div>
      </div>

      <!-- Add Provider Form -->
      <div v-if="showAddForm" class="mt-3 bg-zinc-900 border border-zinc-700 rounded-lg p-4 space-y-3">
        <div class="text-sm font-medium text-zinc-300">Add Custom Provider</div>
        <div class="text-xs text-zinc-500">
          Any OpenAI-compatible API works. Set the base URL and your API key.
        </div>
        <input v-model="newProvider.name" placeholder="Provider name (e.g., nan, deepseek, openai)"
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

    <!-- Supported APIs Reference -->
    <section>
      <h2 class="text-xs font-semibold text-zinc-500 tracking-widest mb-3">SUPPORTED APIS</h2>
      <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4 space-y-3">
        <div class="text-xs text-zinc-400">
          All OpenAI-compatible APIs work. Set <code class="bg-zinc-800 px-1 rounded">base_url</code> and
          <code class="bg-zinc-800 px-1 rounded">api_key</code> in your provider config.
        </div>
        <div class="grid grid-cols-2 gap-2 text-xs">
          <div class="bg-zinc-800 rounded p-2">
            <div class="text-zinc-300 font-medium">OpenAI</div>
            <div class="text-zinc-500 font-mono text-[10px]">api.openai.com/v1</div>
          </div>
          <div class="bg-zinc-800 rounded p-2">
            <div class="text-zinc-300 font-medium">Anthropic</div>
            <div class="text-zinc-500 font-mono text-[10px]">api.anthropic.com</div>
          </div>
          <div class="bg-zinc-800 rounded p-2">
            <div class="text-zinc-300 font-medium">DeepSeek</div>
            <div class="text-zinc-500 font-mono text-[10px]">api.deepseek.com/v1</div>
          </div>
          <div class="bg-zinc-800 rounded p-2">
            <div class="text-zinc-300 font-medium">GitHub Copilot</div>
            <div class="text-zinc-500 font-mono text-[10px]">api.githubcopilot.com</div>
          </div>
          <div class="bg-zinc-800 rounded p-2">
            <div class="text-zinc-300 font-medium">Ollama (local)</div>
            <div class="text-zinc-500 font-mono text-[10px]">localhost:11434/v1</div>
          </div>
          <div class="bg-zinc-800 rounded p-2">
            <div class="text-zinc-300 font-medium">Custom</div>
            <div class="text-zinc-500 font-mono text-[10px]">Any OpenAI-compatible URL</div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>