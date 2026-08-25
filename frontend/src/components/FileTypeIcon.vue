<script setup>
import { computed } from 'vue'
import { File } from 'lucide-vue-next'

const props = defineProps({
  pattern: { type: String, default: '' },
  size: { type: Number, default: 16 },
})

const MAP = {
  pdf: { label: 'PDF', color: 'rgb(var(--danger-6))' },
  doc: { label: 'DOC', color: 'rgb(var(--primary-6))' },
  docx: { label: 'DOC', color: 'rgb(var(--primary-6))' },
  odt: { label: 'DOC', color: 'rgb(var(--primary-6))' },
  rtf: { label: 'DOC', color: 'rgb(var(--primary-6))' },
  xls: { label: 'XLS', color: 'rgb(var(--success-6))' },
  xlsx: { label: 'XLS', color: 'rgb(var(--success-6))' },
  ods: { label: 'XLS', color: 'rgb(var(--success-6))' },
  csv: { label: 'CSV', color: 'rgb(var(--success-6))' },
  ppt: { label: 'PPT', color: 'rgb(var(--warning-6))' },
  pptx: { label: 'PPT', color: 'rgb(var(--warning-6))' },
  odp: { label: 'PPT', color: 'rgb(var(--warning-6))' },
  txt: { label: 'TXT', color: 'var(--color-text-3)' },
  md: { label: 'MD', color: 'rgb(var(--primary-6))' },
  zip: { label: 'ZIP', color: 'rgb(var(--magenta-6))' },
}

const info = computed(() => {
  const ext = String(props.pattern || '').replace(/^\*\./, '').replace(/^\./, '').trim().toLowerCase()
  if (MAP[ext]) return MAP[ext]
  return { label: (ext || 'FILE').slice(0, 3).toUpperCase(), color: 'var(--color-text-3)' }
})

const badgeFont = computed(() => `${Math.max(6, Math.round(props.size * 0.38))}px`)
</script>

<template>
  <div class="file-type-icon" :style="{ width: size + 'px', height: size + 'px' }">
    <File :size="size" class="fti-file" :stroke-width="1.8" />
    <span class="fti-badge" :style="{ background: info.color, fontSize: badgeFont }">{{ info.label }}</span>
  </div>
</template>

<style scoped>
.file-type-icon {
  position: relative;
  display: inline-flex;
  flex-shrink: 0;
}
.fti-file {
  color: var(--color-text-2);
}
.fti-badge {
  position: absolute;
  left: 50%;
  top: 60%;
  transform: translate(-50%, -50%);
  color: #fff;
  font-weight: 700;
  line-height: 1;
  padding: 1px 2px;
  border-radius: 2px;
  white-space: nowrap;
}
</style>
