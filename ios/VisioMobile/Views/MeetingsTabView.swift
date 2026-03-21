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

    var body: some View {
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

    // MARK: - States

    private var onboardingView: some View {
        VStack(spacing: 24) {
            Text("📅")
                .font(.system(size: 64))
            Text("Connectez votre agenda")
                .font(.title2)
                .fontWeight(.semibold)
                .foregroundStyle(VisioColors.onBackground(dark: isDark))
                .multilineTextAlignment(.center)
            Text("Ajoutez l'URL de votre calendrier iCal pour voir vos réunions à venir.")
                .font(.subheadline)
                .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            Button(action: onSettings) {
                Label("Configurer le calendrier", systemImage: "calendar.badge.plus")
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
            Text("Synchronisation...")
                .font(.subheadline)
                .foregroundStyle(VisioColors.secondaryText(dark: isDark))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyView: some View {
        VStack(spacing: 24) {
            Text("🗓️")
                .font(.system(size: 64))
            Text("Aucune réunion à venir")
                .font(.title2)
                .fontWeight(.semibold)
                .foregroundStyle(VisioColors.onBackground(dark: isDark))
                .multilineTextAlignment(.center)
            Button(action: onRefresh) {
                Label("Actualiser", systemImage: "arrow.clockwise")
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
                                onJoin: { onJoinMeeting(meeting) }
                            )
                            .padding(.horizontal, 16)
                            .padding(.vertical, 4)
                        }
                    } header: {
                        Text(group.day)
                            .font(.subheadline)
                            .fontWeight(.semibold)
                            .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 16)
                            .padding(.vertical, 6)
                            .background(VisioColors.background(dark: isDark))
                    }
                }
            }
            .padding(.bottom, 16)
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
        formatter.locale = Locale(identifier: "fr_FR")

        var grouped: [(String, Meeting)] = []
        for meeting in meetings {
            let date = Date(timeIntervalSince1970: TimeInterval(meeting.startTime))
            let dayLabel: String
            if calendar.isDateInToday(date) {
                dayLabel = "Aujourd'hui"
            } else if calendar.isDateInTomorrow(date) {
                dayLabel = "Demain"
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
    let onJoin: () -> Void

    private var now: Date { Date() }

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

    private var timeLabel: String {
        let interval = startDate.timeIntervalSince(now)
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "fr_FR")
        if interval < 4 * 3600 && interval > 0 {
            // Relative
            let minutes = Int(interval / 60)
            if minutes < 60 {
                return "dans \(minutes) min"
            } else {
                let hours = minutes / 60
                let remaining = minutes % 60
                if remaining == 0 {
                    return "dans \(hours)h"
                } else {
                    return "dans \(hours)h\(remaining)"
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

    var body: some View {
        HStack(spacing: 12) {
            // Left accent bar for imminent/in-progress
            if isInProgress || isImminent {
                RoundedRectangle(cornerRadius: 2)
                    .fill(VisioColors.primary500)
                    .frame(width: 3)
                    .padding(.vertical, 4)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(meeting.summary)
                    .font(.body)
                    .fontWeight(.medium)
                    .foregroundStyle(
                        isInProgress || isImminent
                            ? VisioColors.primary500
                            : VisioColors.onSurface(dark: isDark)
                    )
                    .lineLimit(2)

                HStack(spacing: 8) {
                    if isInProgress {
                        Label("En cours", systemImage: "circle.fill")
                            .font(.caption)
                            .foregroundStyle(.green)
                            .labelStyle(SmallLabelStyle())
                    } else {
                        Text(timeLabel)
                            .font(.caption)
                            .foregroundStyle(
                                isImminent
                                    ? VisioColors.primary500
                                    : VisioColors.secondaryText(dark: isDark)
                            )
                    }

                    if !meeting.serverName.isEmpty {
                        Text("·")
                            .font(.caption)
                            .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                        Text(meeting.serverName)
                            .font(.caption)
                            .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                            .lineLimit(1)
                    }
                }
            }

            Spacer()

            Button(action: onJoin) {
                Text("Rejoindre")
                    .font(.subheadline)
                    .fontWeight(.medium)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 7)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(isInProgress || isImminent
                                  ? VisioColors.primary500
                                  : VisioColors.primary500.opacity(0.15))
                    )
                    .foregroundStyle(
                        isInProgress || isImminent
                            ? Color.white
                            : VisioColors.primary500
                    )
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(isDark
                      ? Color(red: 0.12, green: 0.12, blue: 0.18)
                      : Color(red: 0.95, green: 0.95, blue: 0.97))
        )
    }
}

private struct SmallLabelStyle: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 4) {
            configuration.icon.font(.system(size: 6))
            configuration.title
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
        onJoinMeeting: { _ in }
    )
    .environmentObject(VisioManager())
}
