<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from "vue";
import * as echarts from "echarts";

const props = defineProps<{
  download: number[]; // 下行数据点
  upload: number[]; // 上行数据点
  labels: string[];
  theme?: string;
}>();

const chartRef = ref<HTMLDivElement>();
let chart: echarts.ECharts | null = null;

function colors() {
  const light = props.theme === "light";
  return {
    tooltipBg: light ? "rgba(255,255,255,0.96)" : "rgba(23,28,42,0.95)",
    tooltipBorder: light ? "#e3e8f0" : "#2e3648",
    tooltipText: light ? "#1a1f2e" : "#e6e9f0",
    legendText: light ? "#4a5568" : "#9aa3b5",
    axisLine: light ? "#e3e8f0" : "#242b3d",
    axisLabel: light ? "#8a93a6" : "#6b7280",
    splitLine: light ? "#eef1f6" : "#1e2433",
  };
}

function render() {
  if (!chartRef.value) return;
  if (!chart) {
    chart = echarts.init(chartRef.value);
  }
  const c = colors();
  chart.setOption({
    backgroundColor: "transparent",
    grid: { left: 8, right: 8, top: 16, bottom: 8, containLabel: true },
    tooltip: {
      trigger: "axis",
      backgroundColor: c.tooltipBg,
      borderColor: c.tooltipBorder,
      textStyle: { color: c.tooltipText, fontSize: 12 },
    },
    legend: {
      show: true,
      right: 0,
      top: 0,
      textStyle: { color: c.legendText, fontSize: 11 },
      itemWidth: 14,
      itemHeight: 8,
    },
    xAxis: {
      type: "category",
      data: props.labels,
      boundaryGap: false,
      axisLine: { lineStyle: { color: c.axisLine } },
      axisTick: { show: false },
      axisLabel: { color: c.axisLabel, fontSize: 10 },
    },
    yAxis: {
      type: "value",
      splitLine: { lineStyle: { color: c.splitLine } },
      axisLabel: {
        color: c.axisLabel,
        fontSize: 10,
        formatter: (v: number) => (v >= 1024 ? (v / 1024).toFixed(1) + "K" : v + ""),
      },
    },
    series: [
      {
        name: "下行",
        type: "line",
        smooth: true,
        symbol: "none",
        lineStyle: { width: 2, color: "#34d399" },
        areaStyle: {
          color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
            { offset: 0, color: "rgba(52,211,153,0.25)" },
            { offset: 1, color: "rgba(52,211,153,0)" },
          ]),
        },
        data: props.download,
      },
      {
        name: "上行",
        type: "line",
        smooth: true,
        symbol: "none",
        lineStyle: { width: 2, color: "#4f8cff" },
        areaStyle: {
          color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
            { offset: 0, color: "rgba(79,140,255,0.25)" },
            { offset: 1, color: "rgba(79,140,255,0)" },
          ]),
        },
        data: props.upload,
      },
    ],
  });
}

function handleResize() {
  chart?.resize();
}

onMounted(() => {
  render();
  window.addEventListener("resize", handleResize);
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", handleResize);
  chart?.dispose();
  chart = null;
});

watch(
  () => [props.download, props.upload, props.labels, props.theme],
  () => render(),
  { deep: true }
);
</script>

<template>
  <div ref="chartRef" class="chart"></div>
</template>

<style scoped>
.chart {
  width: 100%;
  height: 100%;
  min-height: 220px;
}
</style>
