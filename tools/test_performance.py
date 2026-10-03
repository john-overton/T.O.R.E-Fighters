"""Performance reports must prove that the intended workload actually ran."""
import unittest

import performance


class WorkloadEvidence(unittest.TestCase):
    def report(self):
        return (
            "paused frames: 0; completed camera readbacks: 10\n"
            "workload: 2411 ticks (target 2400); designated estimate frames: 17; weapon: AIM120.JT\n"
            "simulated: 2400 ticks in 2.60 s, 7.69x real time (asked for 8x)\n"
            "frame interval: mean 12.00 ms, p50 10.00, p95 19.00, p99 31.00, max 55.00\n"
        )

    def test_completed_workload_keeps_rate_and_tail_latency(self):
        result = performance.frame_summary(self.report(), 2400, 8)
        self.assertEqual(result["ticks"], 2411)
        self.assertEqual(result["frame_p99_ms"], 31)
        self.assertEqual(result["achieved_scale"], 7.69)

    def test_invalid_cases_cannot_be_reported_as_successful_measurements(self):
        for text in (
            self.report().replace("frames: 17", "frames: 0"),
            self.report().replace("2411 ticks", "1000 ticks"),
            self.report().replace("asked for 8x", "asked for 1x"),
            self.report().replace("weapon: AIM120.JT", "weapon: none"),
            self.report().replace("weapon: AIM120.JT", "weapon: AIM9M.JT"),
            self.report().replace("paused frames: 0", "paused frames: 1"),
            self.report().split("frame interval")[0],
        ):
            with self.subTest(text=text), self.assertRaises(ValueError):
                performance.frame_summary(text, 2400, 8)

    def test_old_host_cpu_report_is_not_an_elapsed_measurement(self):
        with self.assertRaises(ValueError):
            performance.host_summary("host cost: 0.1 ms a tick on average")
        report = (
            "host elapsed: 2.000 ms a tick on average (24.0% of the 120 Hz wall-time budget); "
            "ticking calls p50 1.000 ms, p95 3.000 ms, p99 4.000 ms, p99.9 5.000 ms, longest 6.000 ms"
        )
        self.assertEqual(performance.host_summary(report)["host_mean_ms"], 2)


if __name__ == "__main__":
    unittest.main()
