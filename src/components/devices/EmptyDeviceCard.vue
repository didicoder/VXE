<script setup lang="ts">
/** 空状态占位卡片：和设备卡同尺寸，只显示“待添加”和灰色闪电。 */
import BatteryOrb from '../BatteryOrb.vue';

const { interactive = false } = defineProps<{ interactive?: boolean }>();
const emit = defineEmits<{ add: [] }>();
</script>

<template>
  <article
    class="ecard"
    :class="{ 'ecard--interactive': interactive }"
    :role="interactive ? 'button' : undefined"
    :tabindex="interactive ? 0 : undefined"
    :aria-label="interactive ? '待添加设备，点击添加' : '待添加设备'"
    @click="interactive && emit('add')"
    @keydown.enter="interactive && emit('add')"
    @keydown.space.prevent="interactive && emit('add')"
  >
    <div class="ecard__title">待添加</div>
    <div class="ecard__stage" aria-hidden="true">
      <BatteryOrb class="ecard__orb" tone="none" color="var(--faint)" />
    </div>
  </article>
</template>

<style scoped>
.ecard {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  padding: 6.6% 7% 6.2%;
  overflow: hidden;
  isolation: isolate;
  border: 1px solid var(--card-line);
  border-radius: var(--r-panel);
  background: var(--pcard-bg);
  -webkit-backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  box-shadow: var(--card-shadow);
  -webkit-mask-image: linear-gradient(#000 0 0);
  mask-image: linear-gradient(#000 0 0);
  container-type: inline-size;
}

.ecard--interactive {
  cursor: pointer;
}

.ecard--interactive:hover {
  border-color: var(--card-line-hover);
  box-shadow: var(--card-shadow-hover);
}

.ecard__title {
  color: var(--ink);
  font-family: var(--font-body);
  font-size: clamp(24px, 8cqw, 40px);
  font-weight: 800;
  line-height: 1.1;
  letter-spacing: -0.01em;
}

.ecard__stage {
  display: grid;
  flex: 1;
  min-height: 0;
  place-items: center;
}

.ecard__orb {
  --orb-size: min(58%, 380px);

  opacity: 0.6;
}
</style>