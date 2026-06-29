<script setup lang="ts">
import {
  IconDashboard,
  IconServer,
  IconRobot,
  IconBrain,
  IconTerminal,
  IconSettings,
  IconLogout,
  IconLogin,
  IconWifi,
  IconWifiOff,
  IconCircleDot,
  IconAlertTriangle,
  IconCheck,
  IconX,
  IconPlayerPlay,
  IconPlayerStop,
  IconRefresh,
  IconSearch,
  IconPlus,
  IconTrash,
  IconChevronLeft,
  IconChevronRight,
  IconClock,
  IconTent,
  IconShield,
  IconCode,
  IconList,
  IconEye,
  IconEdit,
  IconCopy,
  IconDownload,
  IconUpload,
  IconSend,
  IconMessage,
  type Icon,
} from '@tabler/icons-vue'

const iconMap: Record<string, Icon> = {
  dashboard: IconDashboard,
  server: IconServer,
  robot: IconRobot,
  brain: IconBrain,
  terminal: IconTerminal,
  settings: IconSettings,
  logout: IconLogout,
  login: IconLogin,
  wifi: IconWifi,
  'wifi-off': IconWifiOff,
  dot: IconCircleDot,
  alert: IconAlertTriangle,
  check: IconCheck,
  x: IconX,
  play: IconPlayerPlay,
  stop: IconPlayerStop,
  refresh: IconRefresh,
  search: IconSearch,
  plus: IconPlus,
  trash: IconTrash,
  'chevron-left': IconChevronLeft,
  'chevron-right': IconChevronRight,
  clock: IconClock,
  token: IconTent,
  shield: IconShield,
  code: IconCode,
  list: IconList,
  eye: IconEye,
  edit: IconEdit,
  copy: IconCopy,
  download: IconDownload,
  upload: IconUpload,
  send: IconSend,
  message: IconMessage,
}

const props = defineProps<{
  name: string
  size?: number
  stroke?: string
  color?: string
}>()

const icon = iconMap[props.name]
</script>

<template>
  <component
    :is="icon"
    :size="size || 18"
    :stroke="stroke || '1.5'"
    :color="color"
    class="inline-block shrink-0"
  />
</template>
