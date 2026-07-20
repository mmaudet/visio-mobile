import type { CSSProperties, ReactNode } from 'react'
import { Avatar } from './Avatar'
import { Icon } from '../Icon'
import { Tag } from './Tag'

export interface VideoTileParticipant {
  name: string
  hasVideo: boolean
  muted?: boolean
  speaking?: boolean
  hand?: boolean
  you?: boolean
  tone?: string
}

export interface VideoTileProps {
  p: VideoTileParticipant
  big?: boolean
  radius?: string | number
  children?: ReactNode
  /** Overlay éléments libres (par ex. flux vidéo réel). */
  videoSlot?: ReactNode
}

export function VideoTile({
  p,
  big,
  radius = 'var(--r-tile)',
  children,
  videoSlot,
}: Readonly<VideoTileProps>) {
  const tone = p.tone || '#3a5bd9'
  const style: CSSProperties = {
    position: 'relative',
    width: '100%',
    height: '100%',
    borderRadius: typeof radius === 'number' ? `${radius}px` : radius,
    overflow: 'hidden',
    background:
      p.hasVideo && !videoSlot
        ? `linear-gradient(150deg, ${tone}, color-mix(in oklab, ${tone} 45%, #05060a))`
        : '#16181d',
    boxShadow: p.speaking
      ? '0 0 0 3px var(--accent)'
      : 'inset 0 0 0 1px rgba(255,255,255,0.06)',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
  }

  return (
    <div style={style}>
      {videoSlot ?? (
        <>
          {p.hasVideo ? (
            <div
              style={{
                width: '100%',
                height: '100%',
                background:
                  'radial-gradient(120% 80% at 70% 25%, rgba(255,255,255,0.18), transparent 55%)',
              }}
            />
          ) : (
            <Avatar name={p.name} size={big ? 84 : 56} />
          )}
        </>
      )}
      {/* name chip */}
      <div
        style={{
          position: 'absolute',
          left: 10,
          bottom: 10,
          display: 'flex',
          alignItems: 'center',
          gap: 5,
          background: 'rgba(8,10,14,0.55)',
          backdropFilter: 'blur(6px)',
          padding: '4px 9px 4px 7px',
          borderRadius: 8,
        }}
      >
        <Icon
          name={p.muted ? 'micOff' : 'mic'}
          size={13}
          color={p.muted ? '#ff6b6f' : '#fff'}
        />
        <span
          style={{
            color: '#fff',
            fontSize: 12,
            fontWeight: 600,
            maxWidth: big ? 200 : 110,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {p.you ? 'Vous' : p.name}
        </span>
      </div>
      {p.speaking && (
        <div style={{ position: 'absolute', top: 10, right: 10 }}>
          <Tag tone="live" dot>
            Parle
          </Tag>
        </div>
      )}
      {p.hand && (
        <div
          style={{
            position: 'absolute',
            top: 10,
            left: 10,
            width: 26,
            height: 26,
            borderRadius: 8,
            background: 'var(--warn)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <Icon name="hand" size={15} color="#fff" />
        </div>
      )}
      {children}
    </div>
  )
}

export default VideoTile
