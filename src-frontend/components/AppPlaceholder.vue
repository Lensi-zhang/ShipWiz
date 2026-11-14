<script setup lang="ts">
import { ArrowLeft, Check, Compass } from "lucide-vue-next";
import { dict } from "../dict";

defineProps<{
  title: string;
  description: string;
  phase: string;
  features: readonly string[];
}>();
</script>

<template>
  <div class="placeholder-page">
    <div class="page-heading">
      <span class="eyebrow">{{ phase }}</span>
      <h1>{{ title }}</h1>
      <p>{{ description }}</p>
    </div>
    <div class="placeholder-chart">
      <div class="route-line" />
      <div class="compass-seal"><Compass :size="42" :stroke-width="1.2" /></div>
      <span class="planning-badge">{{ dict.placeholder.badge }}</span>
      <h2>{{ dict.placeholder.title }}</h2>
      <div class="feature-list">
        <span class="feature-caption">{{ dict.placeholder.featuresTitle }}</span>
        <div v-for="feature in features" :key="feature" class="feature-item">
          <Check :size="16" />{{ feature }}
        </div>
      </div>
      <router-link to="/fleet" class="back-link">
        <ArrowLeft :size="16" />{{ dict.placeholder.back }}
      </router-link>
    </div>
  </div>
</template>

<style scoped>
.placeholder-chart { position: relative; min-height: 490px; padding: 64px 10%; overflow: hidden; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); }
.placeholder-chart::before { content: ""; position: absolute; inset: 0; opacity: .32; background-image: linear-gradient(var(--sw-border) 1px, transparent 1px), linear-gradient(90deg, var(--sw-border) 1px, transparent 1px); background-size: 36px 36px; -webkit-mask-image: linear-gradient(to right, var(--sw-text), transparent 80%); mask-image: linear-gradient(to right, var(--sw-text), transparent 80%); }
.placeholder-chart > * { position: relative; }
.route-line { position: absolute; top: 45px; right: 8%; width: 45%; height: 160px; border: 1px dashed var(--sw-primary); border-color: var(--sw-primary) transparent transparent var(--sw-primary); border-radius: 50%; transform: rotate(-8deg); }
.compass-seal { display: grid; place-items: center; width: 78px; height: 78px; margin-bottom: 25px; color: var(--sw-primary); border: 1px solid var(--sw-primary); border-radius: 50%; background: var(--sw-primary-soft); }
.planning-badge { color: var(--sw-warning); font-size: 10px; font-weight: 700; letter-spacing: 2px; }
.placeholder-chart h2 { margin: 10px 0 30px; color: var(--sw-text); font-family: Georgia, "Noto Sans SC", serif; font-size: 25px; font-weight: 500; }
.feature-list { display: flex; flex-direction: column; align-items: flex-start; gap: 12px; }
.feature-caption { margin-bottom: 3px; color: var(--sw-muted); font-size: 11px; }
.feature-item { display: flex; align-items: center; gap: 9px; color: var(--sw-text-secondary); font-size: 13px; }
.feature-item svg { color: var(--sw-primary); }
.back-link { display: inline-flex; align-items: center; gap: 7px; margin-top: 34px; color: var(--sw-primary); font-size: 12px; transition: color 300ms; }
.back-link:hover { color: var(--sw-text); text-decoration: underline; }
</style>
