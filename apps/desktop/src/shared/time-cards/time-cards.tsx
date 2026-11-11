/** The activity calendar and hours-of-week cards, shared by the Overview and Activity. */
import type { AccuracyClass, CalendarDay, HourCell } from "@veyra/contracts";

import { CalendarHeatmap, HourHeatmap, HourTable, RankTable } from "../charts";
import { fmt } from "../format";
import { ChartCard } from "../ui";

export function CalendarCard({
  calendar,
  accuracy,
  onSelectDate,
}: {
  calendar: readonly CalendarDay[];
  accuracy: AccuracyClass | null;
  onSelectDate: (date: string) => void;
}) {
  const days = calendar.map((d) => ({ date: d.date, value: d.tokens }));
  return (
    <ChartCard
      title="Activity calendar"
      definition="Tokens per local day over the year ending on the last day of the period. Click a day to narrow the period to it."
      accuracy={accuracy}
      chart={
        <CalendarHeatmap
          days={days}
          format={fmt.compact}
          label="Tokens per day over the last year"
          onSelectDate={onSelectDate}
        />
      }
      table={
        <RankTable
          items={days
            .filter((d) => d.value > 0)
            .reverse()
            .map((d) => ({
              key: d.date,
              label: fmt.day(d.date),
              value: d.value,
              share: 0,
              color: "",
            }))}
          format={fmt.integer}
          caption="Tokens per day"
          header="Day"
        />
      }
    />
  );
}

export function HoursCard({
  hours,
  accuracy,
}: {
  hours: readonly HourCell[];
  accuracy: AccuracyClass | null;
}) {
  const cells = hours.map((h) => ({ weekday: h.weekday, hour: h.hour, value: h.tokens }));
  return (
    <ChartCard
      title="Hours of the week"
      definition="Tokens by local weekday and hour over the selected period."
      accuracy={accuracy}
      chart={<HourHeatmap cells={cells} format={fmt.compact} label="Tokens by weekday and hour" />}
      table={<HourTable cells={cells} format={fmt.integer} caption="Tokens by weekday and hour" />}
    />
  );
}
