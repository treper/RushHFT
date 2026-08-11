import uPlot from 'uplot';

// Theme colors from app.css
const COLORS = {
  bg: '#0d1117',
  panel: '#161b22',
  border: '#30363d',
  muted: '#8b949e',
  accent: '#58a6ff',
  bid: '#f85149',
  ask: '#7ee787',
};

export function baseOptions(width: number, height: number, series: uPlot.Series[], scales: Record<string, { range?: (u: uPlot, min: number, max: number) => [number, number] }> = {}): uPlot.Options {
  return {
    width,
    height,
    series,
    scales: { x: { time: true }, y: { ...scales.y } },
    axes: [
      { grid: { stroke: COLORS.border }, ticks: { stroke: COLORS.border }, stroke: COLORS.muted },
      { grid: { stroke: COLORS.border }, ticks: { stroke: COLORS.border }, stroke: COLORS.muted },
    ],
    cursor: { show: true },
    legend: { show: false },
  };
}

export function lineSeries(label: string, color: string): uPlot.Series {
  return {
    label,
    stroke: color,
    points: { show: false },
    width: 1,
  };
}

export function stepSeries(label: string, color: string): uPlot.Series {
  return {
    label,
    stroke: color,
    points: { show: false },
    width: 1,
    spanGaps: true,
  };
}
