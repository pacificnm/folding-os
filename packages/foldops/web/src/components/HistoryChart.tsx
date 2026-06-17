import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { HistoryPoint } from "../types";

export interface ChartSeries {
  key: keyof HistoryPoint;
  name: string;
  color: string;
  unit?: string;
  domain?: [number | string, number | string];
}

interface HistoryChartProps {
  title: string;
  data: HistoryPoint[];
  series: ChartSeries[];
  height?: number;
}

function formatValue(value: number, unit?: string): string {
  if (unit === "°C") return `${value.toFixed(1)}°C`;
  if (unit === "%") return `${value.toFixed(1)}%`;
  if (unit === "ppd") {
    if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
    if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`;
    return value.toFixed(0);
  }
  return value.toFixed(1);
}

export function HistoryChart({
  title,
  data,
  series,
  height = 220,
}: HistoryChartProps) {
  const hasData = series.some((s) =>
    data.some((d) => d[s.key] != null && typeof d[s.key] === "number"),
  );

  return (
    <section className="chart-panel">
      <h3 className="chart-title">{title}</h3>
      {!hasData ? (
        <p className="chart-empty">No data in this range</p>
      ) : (
        <ResponsiveContainer width="100%" height={height}>
          <LineChart
            data={data}
            margin={{ top: 8, right: 12, left: 0, bottom: 4 }}
          >
            <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" />
            <XAxis
              dataKey="label"
              tick={{ fill: "var(--muted)", fontSize: 11 }}
              tickLine={false}
              axisLine={{ stroke: "var(--border)" }}
              minTickGap={40}
            />
            <YAxis
              tick={{ fill: "var(--muted)", fontSize: 11 }}
              tickLine={false}
              axisLine={{ stroke: "var(--border)" }}
              domain={series.length === 1 ? series[0].domain : undefined}
              width={48}
            />
            <Tooltip
              contentStyle={{
                background: "var(--surface-2)",
                border: "1px solid var(--border)",
                borderRadius: 8,
                fontSize: 12,
              }}
              labelStyle={{ color: "var(--muted)" }}
              formatter={(value, name) => {
                if (typeof value !== "number") return ["—", String(name)];
                const s = series.find((x) => x.name === name);
                return [formatValue(value, s?.unit), String(name)];
              }}
              labelFormatter={(_, payload) => {
                const row = payload?.[0]?.payload as HistoryPoint | undefined;
                return row?.time
                  ? new Date(row.time).toLocaleString()
                  : "";
              }}
            />
            {series.map((s) => (
              <Line
                key={s.key}
                type="monotone"
                dataKey={s.key}
                name={s.name}
                stroke={s.color}
                strokeWidth={2}
                dot={false}
                connectNulls
                isAnimationActive={false}
              />
            ))}
          </LineChart>
        </ResponsiveContainer>
      )}
    </section>
  );
}
