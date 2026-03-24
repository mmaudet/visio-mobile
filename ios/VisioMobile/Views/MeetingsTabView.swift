import Combine
import SwiftUI
import visioFFI

struct MeetingsTabView: View {
    let meetings: [Meeting]
    let hasCalendarUrl: Bool
    let isLoading: Bool
    let isDark: Bool
    let lang: String
    let onSettings: () -> Void
    let onRefresh: () -> Void
    let onJoinMeeting: (Meeting) -> Void

    // Fix 3: live-updating date so countdowns and imminent state refresh
    @State private var now = Date()
    private let timer = Timer.publish(every: 60, on: .main, in: .common).autoconnect()

    var body: some View {
        Group {
            if !hasCalendarUrl {
                onboardingView
            } else if isLoading && meetings.isEmpty {
                loadingView
            } else if meetings.isEmpty {
                emptyView
            } else {
                listView
            }
        }
        .onReceive(timer) { _ in now = Date() }
    }

    // MARK: - States

    private var onboardingView: some View {
        VStack(spacing: 24) {
            Text("\u{1F4C5}")
                .font(.system(size: 64))
            Text(Strings.t("meetings.onboarding", lang: lang))
                .font(.title2)
                .fontWeight(.semibold)
                .foregroundStyle(VisioColors.onBackground(dark: isDark))
                .multilineTextAlignment(.center)
            Text(Strings.t("meetings.onboarding.hint", lang: lang))
                .font(.subheadline)
                .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            Button(action: onSettings) {
                Label(Strings.t("meetings.onboarding.configure", lang: lang),
                      systemImage: "calendar.badge.plus")
                    .font(.headline)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
            }
            .buttonStyle(.borderedProminent)
            .tint(VisioColors.primary500)
            .padding(.horizontal, 48)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .scaleEffect(1.4)
            Text(Strings.t("meetings.loading", lang: lang))
                .font(.subheadline)
                .foregroundStyle(VisioColors.secondaryText(dark: isDark))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyView: some View {
        VStack(spacing: 24) {
            Text("\u{2600}\u{FE0F}")
                .font(.system(size: 64))
            Text(Strings.t("meetings.empty", lang: lang))
                .font(.title2)
                .fontWeight(.semibold)
                .foregroundStyle(VisioColors.onBackground(dark: isDark))
                .multilineTextAlignment(.center)
            Text(Strings.t("meetings.empty.subtitle", lang: lang))
                .font(.subheadline)
                .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                .multilineTextAlignment(.center)
            Button(action: onRefresh) {
                Label(Strings.t("meetings.refresh", lang: lang), systemImage: "arrow.clockwise")
                    .font(.subheadline)
            }
            .foregroundStyle(VisioColors.primary500)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var listView: some View {
        ScrollView {
            LazyVStack(spacing: 0, pinnedViews: .sectionHeaders) {
                ForEach(groupedMeetings, id: \.day) { group in
                    Section {
                        ForEach(group.meetings, id: \.id) { meeting in
                            MeetingRow(
                                meeting: meeting,
                                isDark: isDark,
                                lang: lang,
                                currentDate: now,
                                onJoin: { onJoinMeeting(meeting) }
                            )
                            .padding(.horizontal, 16)
                            .padding(.vertical, 4)
                        }
                    } header: {
                        Text(group.day.uppercased())
                            .font(.caption)
                            .fontWeight(.semibold)
                            .tracking(1)
                            .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 16)
                            .padding(.vertical, 6)
                            .background(VisioColors.background(dark: isDark))
                    }
                }
            }

            // Sync footer
            Text(Strings.t("meetings.sync", lang: lang).replacingOccurrences(of: "{time}", with: "< 1 min"))
                .font(.caption2)
                .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                .opacity(0.7)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 12)
        }
        .refreshable {
            onRefresh()
            // Allow the refresh indicator to show briefly
            try? await Task.sleep(nanoseconds: 500_000_000)
        }
    }

    // MARK: - Grouping

    private struct DayGroup {
        let day: String
        let meetings: [Meeting]
    }

    private var groupedMeetings: [DayGroup] {
        let calendar = Calendar.current
        let formatter = DateFormatter()
        formatter.locale = lang == "fr" ? Locale(identifier: "fr_FR") : Locale(identifier: "en_US")

        var grouped: [(String, Meeting)] = []
        for meeting in meetings {
            let date = Date(timeIntervalSince1970: TimeInterval(meeting.startTime))
            let dayLabel: String
            if calendar.isDateInToday(date) {
                dayLabel = Strings.t("meetings.today", lang: lang)
            } else if calendar.isDateInTomorrow(date) {
                dayLabel = Strings.t("meetings.tomorrow", lang: lang)
            } else {
                formatter.dateFormat = "EEEE d MMMM"
                dayLabel = formatter.string(from: date).capitalized
            }
            grouped.append((dayLabel, meeting))
        }

        var result: [DayGroup] = []
        var current: [Meeting] = []
        var currentDay: String = ""
        for (day, meeting) in grouped {
            if day != currentDay {
                if !current.isEmpty {
                    result.append(DayGroup(day: currentDay, meetings: current))
                }
                current = [meeting]
                currentDay = day
            } else {
                current.append(meeting)
            }
        }
        if !current.isEmpty {
            result.append(DayGroup(day: currentDay, meetings: current))
        }
        return result
    }
}

// MARK: - Meeting Row

private struct MeetingRow: View {
    let meeting: Meeting
    let isDark: Bool
    let lang: String
    var currentDate: Date = Date()
    let onJoin: () -> Void

    private var now: Date { currentDate }

    private var startDate: Date {
        Date(timeIntervalSince1970: TimeInterval(meeting.startTime))
    }
    private var endDate: Date {
        Date(timeIntervalSince1970: TimeInterval(meeting.endTime))
    }

    private var isInProgress: Bool {
        now >= startDate && now < endDate
    }

    private var isImminent: Bool {
        !isInProgress && startDate.timeIntervalSince(now) < 15 * 60 && startDate > now
    }

    private var isAccent: Bool {
        isInProgress || isImminent
    }

    private var timeLabel: String {
        let interval = startDate.timeIntervalSince(now)
        let formatter = DateFormatter()
        formatter.locale = lang == "fr" ? Locale(identifier: "fr_FR") : Locale(identifier: "en_US")

        if isInProgress {
            formatter.dateFormat = "HH:mm"
            let endStr = formatter.string(from: endDate)
            let untilStr = Strings.t("meetings.time.until", lang: lang).replacingOccurrences(of: "{time}", with: endStr)
            let inProgressStr = Strings.t("meetings.time.inProgress", lang: lang)
            return "\(inProgressStr) \u{00B7} \(untilStr)"
        }

        if interval < 4 * 3600 && interval > 0 {
            let minutes = Int(interval / 60)
            if minutes < 60 {
                return Strings.t("meetings.time.inMinutes", lang: lang).replacingOccurrences(of: "{minutes}", with: "\(minutes)")
            } else {
                let hours = minutes / 60
                let remaining = minutes % 60
                if remaining == 0 {
                    return Strings.t("meetings.time.inHours", lang: lang).replacingOccurrences(of: "{hours}", with: "\(hours)")
                } else {
                    return Strings.t("meetings.time.inHoursMinutes", lang: lang).replacingOccurrences(of: "{hours}", with: "\(hours)").replacingOccurrences(of: "{minutes}", with: String(format: "%02d", remaining))
                }
            }
        } else if Calendar.current.isDateInToday(startDate) {
            formatter.dateFormat = "HH:mm"
            return formatter.string(from: startDate)
        } else {
            formatter.dateFormat = "EEE HH:mm"
            return formatter.string(from: startDate).capitalized
        }
    }

    private var accentGradient: LinearGradient {
        LinearGradient(
            colors: [VisioColors.primary500, Color(red: 0.36, green: 0.24, blue: 0.86)],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    if isAccent {
                        PulsingDot(
                            color: isInProgress
                                ? Color(red: 0.94, green: 0.25, blue: 0.24)
                                : Color(red: 0.29, green: 0.87, blue: 0.5)
                        )
                    }
                    Text(meeting.summary)
                        .font(.body)
                        .fontWeight(.bold)
                        .foregroundStyle(isAccent ? .white : VisioColors.onSurface(dark: isDark))
                        .lineLimit(2)
                }

                Text(timeLabel)
                    .font(.caption)
                    .foregroundStyle(
                        isAccent
                            ? Color.white.opacity(0.8)
                            : VisioColors.secondaryText(dark: isDark)
                    )

                if !meeting.serverName.isEmpty {
                    Text(meeting.serverName)
                        .font(.caption)
                        .foregroundStyle(
                            isAccent
                                ? Color.white.opacity(0.8)
                                : VisioColors.secondaryText(dark: isDark)
                        )
                        .lineLimit(1)
                }
            }

            Spacer()

            Button(action: onJoin) {
                Text(Strings.t("home.tab.join", lang: lang))
                    .font(.subheadline)
                    .fontWeight(.semibold)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 7)
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .stroke(isAccent ? Color.white : VisioColors.primary500, lineWidth: 1.5)
                    )
                    .foregroundStyle(isAccent ? .white : VisioColors.primary500)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(
            Group {
                if isAccent {
                    RoundedRectangle(cornerRadius: 10)
                        .fill(accentGradient)
                } else {
                    RoundedRectangle(cornerRadius: 10)
                        .fill(isDark
                              ? Color(red: 0.12, green: 0.12, blue: 0.18)
                              : Color(red: 0.95, green: 0.95, blue: 0.97))
                }
            }
        )
    }
}

private struct PulsingDot: View {
    var color: Color = Color(red: 0.29, green: 0.87, blue: 0.5)
    @State private var isAnimating = false

    var body: some View {
        Circle()
            .fill(color)
            .frame(width: 8, height: 8)
            .opacity(isAnimating ? 0.3 : 1.0)
            .onAppear {
                withAnimation(.easeInOut(duration: 0.75).repeatForever(autoreverses: true)) {
                    isAnimating = true
                }
            }
    }
}

#Preview {
    MeetingsTabView(
        meetings: [],
        hasCalendarUrl: false,
        isLoading: false,
        isDark: false,
        lang: "fr",
        onSettings: {},
        onRefresh: {},
        onJoinMeeting: { _ in }
    )
    .environmentObject(VisioManager())
}
