import uPlot from 'uplot';
import { lineSeries, stepSeries, baseOptions } from './uPlotSetup';

// Mirror of Rust ChartPointDto. Decimal fields are strings (rust_decimal default).
export interface ChartPointDto {
  t: number;
  value: string;
  bid: string | null;
  ask: string | null;
  mid: string | null;
}

export function buildSpreadOptions(width: number, height: number): uPlot.Options {
  return baseOptions(width, height, [lineSeries('Spread', '#58a6ff')]);
}

export function spreadData(pts: ChartPointDto[]): uPlot.AlignedData {
  return [
    pts.map((p) => p.t / 1000),
    pts.map((p) => Number(p.value)),
  ];
}

export function buildPriceOptions(width: number, height: number): uPlot.Options {
  return baseOptions(width, height, [
    lineSeries('Bid', '#f85149'),
    lineSeries('Ask', '#7ee787'),
    lineSeries('Mid', '#8b949e'),
  ]);
}

export function priceData(pts: ChartPointDto[]): uPlot.AlignedData {
  return [
    pts.map((p) => p.t / 1000),
    pts.map((p) => Number(p.bid ?? '0')),
    pts.map((p) => Number(p.ask ?? '0')),
    pts.map((p) => Number(p.mid ?? '0')),
  ];
}

export function buildCumulativeOptions(width: number, height: number, label: string, color: string): uPlot.Options {
  return baseOptions(width, height, [stepSeries(label, color)]);
}

export function cumulativeData(pts: ChartPointDto[]): uPlot.AlignedData {
  return [
    pts.map((p) => p.t / 1000),
    pts.map((p) => Number(p.value)),
  ];
}
