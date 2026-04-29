import React, { useEffect, useLayoutEffect, useRef, useState } from 'react'
import styles from '../styles/Mode.module.css'

interface PositionedItem {
  position: number
}

interface ExitItem<T> {
  key: string
  item: T
  rect: DOMRect
  color: string
}

export default function QueueList<T extends PositionedItem>({
  items,
  getKey,
  getGroupKey,
  getColor,
  renderItem,
  renderActions,
  onReorder,
  onTopDrop,
  onRemoveDrop,
  canReorder = true,
  reorderScope = 'all',
  topDropLabel = 'Top',
  removeDropLabel = 'Remove',
  pulseKey,
  pulseToken = 0,
}: {
  items: T[]
  getKey: (item: T) => string
  getGroupKey?: (item: T) => string
  getColor?: (item: T) => string
  renderItem: (item: T) => React.ReactNode
  renderActions?: (item: T) => React.ReactNode
  onReorder: (item: T, toPosition: number) => void
  onTopDrop?: (item: T) => void
  onRemoveDrop?: (item: T) => void
  canReorder?: boolean
  reorderScope?: 'all' | 'group'
  topDropLabel?: string
  removeDropLabel?: string
  pulseKey?: string | null
  pulseToken?: number
}) {
  const [dragKey, setDragKey] = useState<string | null>(null)
  const [dragDelta, setDragDelta] = useState(0)
  const [insertIdx, setInsertIdx] = useState(0)
  const [topHot, setTopHot] = useState(false)
  const [removeHot, setRemoveHot] = useState(false)
  const [exitItems, setExitItems] = useState<ExitItem<T>[]>([])

  const itemElsRef = useRef<Map<string, HTMLElement>>(new Map())
  const topZoneRef = useRef<HTMLDivElement | null>(null)
  const removeZoneRef = useRef<HTMLDivElement | null>(null)
  const pointerIdRef = useRef<number | null>(null)
  const holdTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const scrollRafRef = useRef<number | null>(null)
  const dragRafRef = useRef<number | null>(null)
  const dragKeyRef = useRef<string | null>(null)
  const insertIdxRef = useRef(0)
  const deltaPendingRef = useRef(0)
  const startYRef = useRef(0)
  const lastYRef = useRef(0)
  const movedRef = useRef(false)
  const originalRectsRef = useRef<Map<string, DOMRect>>(new Map())
  const previousRectsRef = useRef<Map<string, DOMRect>>(new Map())
  const previousItemsRef = useRef<T[]>([])
  const previousKeysRef = useRef<string[]>([])
  const scrollAtStartRef = useRef(0)
  const getKeyRef = useRef(getKey)
  const getColorRef = useRef(getColor)
  const wasDraggingRef = useRef(false)
  const touchMoveCleanupRef = useRef<(() => void) | null>(null)
  const listRef = useRef<HTMLOListElement | null>(null)

  dragKeyRef.current = dragKey
  getKeyRef.current = getKey
  getColorRef.current = getColor

  function partnersFor(draggedItem: T): T[] {
    const draggedKey = getKey(draggedItem)
    if (reorderScope === 'all' || !getGroupKey) {
      return items.filter((item) => getKey(item) !== draggedKey)
    }

    const draggedGroup = getGroupKey(draggedItem)
    return items.filter((item) => getGroupKey(item) === draggedGroup && getKey(item) !== draggedKey)
  }

  function computeInsertIdx(pointerY: number, draggedItem: T): number {
    const partners = partnersFor(draggedItem)
    const scrollDelta = window.scrollY - scrollAtStartRef.current
    for (let ci = 0; ci < partners.length; ci++) {
      const rect = originalRectsRef.current.get(getKey(partners[ci]))
      if (!rect) continue
      if (pointerY < rect.top - scrollDelta + rect.height / 2) return ci
    }
    return partners.length
  }

  function getShift(item: T, draggedItem: T, insertIdxVal: number): number {
    const partners = partnersFor(draggedItem)
    const partnerIndex = partners.findIndex((candidate) => getKey(candidate) === getKey(item))
    if (partnerIndex === -1) return 0

    const currentPosition = draggedItem.position
    const ownRect = originalRectsRef.current.get(getKey(item))
    if (!ownRect) return 0

    if (currentPosition < insertIdxVal && partnerIndex >= currentPosition && partnerIndex < insertIdxVal) {
      const predecessor = partnerIndex === currentPosition ? draggedItem : partners[partnerIndex - 1]
      const predecessorRect = originalRectsRef.current.get(getKey(predecessor))
      return predecessorRect ? predecessorRect.top - ownRect.top : 0
    }

    if (currentPosition > insertIdxVal && partnerIndex >= insertIdxVal && partnerIndex < currentPosition) {
      const successor = partnerIndex === currentPosition - 1 ? draggedItem : partners[partnerIndex + 1]
      const successorRect = originalRectsRef.current.get(getKey(successor))
      return successorRect ? successorRect.top - ownRect.top : 0
    }

    return 0
  }

  function startAutoScroll() {
    if (scrollRafRef.current !== null) cancelAnimationFrame(scrollRafRef.current)
    const threshold = 80
    const speed = 10
    function tick() {
      const y = lastYRef.current
      if (y < threshold) {
        window.scrollBy(0, -Math.round(((threshold - y) / threshold) * speed))
      } else if (y > window.innerHeight - threshold) {
        window.scrollBy(0, Math.round(((y - (window.innerHeight - threshold)) / threshold) * speed))
      }
      scrollRafRef.current = requestAnimationFrame(tick)
    }
    scrollRafRef.current = requestAnimationFrame(tick)
  }

  function stopAutoScroll() {
    if (scrollRafRef.current !== null) {
      cancelAnimationFrame(scrollRafRef.current)
      scrollRafRef.current = null
    }
  }

  function resetDrag() {
    if (holdTimerRef.current !== null) {
      clearTimeout(holdTimerRef.current)
      holdTimerRef.current = null
    }
    if (dragRafRef.current !== null) {
      cancelAnimationFrame(dragRafRef.current)
      dragRafRef.current = null
    }
    pointerIdRef.current = null
    movedRef.current = false
    dragKeyRef.current = null
    insertIdxRef.current = 0
    deltaPendingRef.current = 0
    setDragKey(null)
    setDragDelta(0)
    setInsertIdx(0)
    setTopHot(false)
    setRemoveHot(false)
    stopAutoScroll()
    touchMoveCleanupRef.current?.()
    touchMoveCleanupRef.current = null
  }

  useEffect(() => resetDrag, [])

  useEffect(() => {
    if (!canReorder) return
    const el = listRef.current
    if (!el) return
    // A non-passive touchstart listener forces Chrome to disable its compositor-thread
    // fast-scroll optimization, so our non-passive touchmove preventDefault() can work.
    const noop: EventListener = () => { /* non-passive registration only */ }
    el.addEventListener('touchstart', noop, { passive: false })
    return () => { el.removeEventListener('touchstart', noop) }
  }, [canReorder])

  useEffect(() => {
    if (!pulseKey) return

    const el = itemElsRef.current.get(pulseKey)
    if (!el) return

    const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    el.scrollIntoView({ behavior: reduceMotion ? 'auto' : 'smooth', block: 'center' })
    el.animate(
      [
        {
          backgroundColor: 'rgba(29, 185, 84, 0.28)',
          boxShadow: '0 0 0 0 rgba(29, 185, 84, 0.65)',
          borderColor: 'rgba(29, 185, 84, 0.75)',
        },
        {
          backgroundColor: 'rgba(29, 185, 84, 0.18)',
          boxShadow: '0 0 0 9px rgba(29, 185, 84, 0)',
          borderColor: 'rgba(29, 185, 84, 0.45)',
        },
        {
          backgroundColor: 'var(--surface-2)',
          boxShadow: '0 0 0 0 rgba(29, 185, 84, 0)',
          borderColor: 'var(--border-subtle)',
        },
      ],
      {
        duration: reduceMotion ? 120 : 900,
        easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
      },
    )
  }, [pulseKey, pulseToken])

  useLayoutEffect(() => {
    if (dragKeyRef.current) {
      wasDraggingRef.current = true
      previousRectsRef.current = snapshotRects()
      previousItemsRef.current = items
      previousKeysRef.current = items.map(getKeyRef.current)
      return
    }

    const wasDragging = wasDraggingRef.current
    wasDraggingRef.current = false

    const previousRects = previousRectsRef.current
    const previousItems = previousItemsRef.current
    const keyFor = getKeyRef.current
    const colorFor = getColorRef.current
    const orderedKeys = items.map(keyFor)
    const currentKeys = new Set(orderedKeys)
    const keyOrderChanged =
      orderedKeys.length !== previousKeysRef.current.length
      || orderedKeys.some((key, index) => key !== previousKeysRef.current[index])

    if (!keyOrderChanged) {
      previousRectsRef.current = snapshotRects()
      previousItemsRef.current = items
      previousKeysRef.current = orderedKeys
      return
    }

    const removedItems = previousItems
      .map((item) => {
        const key = keyFor(item)
        const rect = previousRects.get(key)
        if (currentKeys.has(key) || !rect) return null
        return {
          key,
          item,
          rect,
          color: colorFor?.(item) ?? 'var(--border-subtle)',
        }
      })
      .filter((item): item is ExitItem<T> => item !== null)

    if (removedItems.length > 0) {
      setExitItems((current) => [...current, ...removedItems])
    }

    for (const item of items) {
      const key = keyFor(item)
      const el = itemElsRef.current.get(key)
      if (!el) continue

      const newRect = el.getBoundingClientRect()
      const oldRect = previousRects.get(key)
      if (oldRect && !wasDragging) {
        const deltaY = oldRect.top - newRect.top
        if (Math.abs(deltaY) > 1) {
          el.getAnimations().forEach((animation) => { animation.cancel() })
          el.animate(
            [
              { transform: `translate3d(0, ${deltaY.toString()}px, 0)` },
              { transform: 'translate3d(0, 0, 0)' },
            ],
            {
              duration: 240,
              easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
            },
          )
        }
      } else {
        el.getAnimations().forEach((animation) => { animation.cancel() })
        el.animate(
          [
            { opacity: 0, transform: 'translate3d(0, 10px, 0) scale(0.98)' },
            { opacity: 1, transform: 'translate3d(0, 0, 0) scale(1)' },
          ],
          {
            duration: 220,
            easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
          },
        )
      }
    }

    previousRectsRef.current = snapshotRects()
    previousItemsRef.current = items
    previousKeysRef.current = orderedKeys
  }, [items])

  function snapshotRects(): Map<string, DOMRect> {
    const rects = new Map<string, DOMRect>()
    for (const [key, el] of itemElsRef.current) {
      rects.set(key, el.getBoundingClientRect())
    }
    return rects
  }

  function activate(el: HTMLElement, item: T, pointerId: number) {
    if (!canReorder) return
    try { el.setPointerCapture(pointerId) } catch { /* ignore */ }

    originalRectsRef.current.clear()
    for (const [key, itemEl] of itemElsRef.current) {
      originalRectsRef.current.set(key, itemEl.getBoundingClientRect())
    }
    scrollAtStartRef.current = window.scrollY

    const key = getKey(item)
    dragKeyRef.current = key
    insertIdxRef.current = item.position
    setDragKey(key)
    setDragDelta(0)
    setInsertIdx(item.position)
    startAutoScroll()
  }

  function handlePointerDown(e: React.PointerEvent<HTMLLIElement>, item: T) {
    if (!canReorder) return
    if ((e.target as HTMLElement).closest('button')) return
    if (e.pointerType === 'mouse' && e.button !== 0) return

    pointerIdRef.current = e.pointerId
    startYRef.current = e.clientY
    lastYRef.current = e.clientY
    movedRef.current = false

    const el = e.currentTarget
    if (e.pointerType === 'touch') {
      const onTouchMove = (te: TouchEvent) => {
        if (dragKeyRef.current !== null) te.preventDefault()
      }
      touchMoveCleanupRef.current?.()
      document.addEventListener('touchmove', onTouchMove, { passive: false })
      touchMoveCleanupRef.current = () => { document.removeEventListener('touchmove', onTouchMove) }

      holdTimerRef.current = setTimeout(() => {
        holdTimerRef.current = null
        if (!movedRef.current && pointerIdRef.current === e.pointerId) activate(el, item, e.pointerId)
      }, 350)
    } else {
      e.preventDefault()
      activate(el, item, e.pointerId)
    }
  }

  function handlePointerMove(e: React.PointerEvent<HTMLLIElement>, item: T) {
    if (!canReorder || pointerIdRef.current !== e.pointerId) return
    lastYRef.current = e.clientY

    if (holdTimerRef.current !== null) {
      if (Math.abs(e.clientY - startYRef.current) > 8) movedRef.current = true
      return
    }

    if (!dragKeyRef.current || getKey(item) !== dragKeyRef.current) return
    deltaPendingRef.current = e.clientY - startYRef.current

    setTopHot(isPointerOverTopZone(e.clientX, e.clientY))
    setRemoveHot(isPointerOverRemoveZone(e.clientX, e.clientY))

    const draggedItem = items.find((candidate) => getKey(candidate) === dragKeyRef.current)
    if (draggedItem) {
      insertIdxRef.current = computeInsertIdx(e.clientY, draggedItem)
    }

    dragRafRef.current ??= requestAnimationFrame(() => {
      dragRafRef.current = null
      setDragDelta(deltaPendingRef.current)
      setInsertIdx(insertIdxRef.current)
    })
  }

  function handlePointerUp(e: React.PointerEvent<HTMLLIElement>, item: T) {
    if (!canReorder || pointerIdRef.current !== e.pointerId) return

    if (holdTimerRef.current !== null) {
      clearTimeout(holdTimerRef.current)
      holdTimerRef.current = null
      pointerIdRef.current = null
      movedRef.current = false
      return
    }

    if (dragKeyRef.current && getKey(item) === dragKeyRef.current) {
      const draggedItem = items.find((candidate) => getKey(candidate) === dragKeyRef.current)
      const finalInsertIdx = insertIdxRef.current
      if (draggedItem && onTopDrop && isPointerOverTopZone(e.clientX, e.clientY)) {
        onTopDrop(draggedItem)
      } else if (draggedItem && onRemoveDrop && isPointerOverRemoveZone(e.clientX, e.clientY)) {
        onRemoveDrop(draggedItem)
      } else if (draggedItem && finalInsertIdx !== draggedItem.position) {
        onReorder(draggedItem, finalInsertIdx)
      }
    }

    resetDrag()
  }

  function handlePointerCancel(e: React.PointerEvent<HTMLLIElement>) {
    if (!canReorder || pointerIdRef.current !== e.pointerId) return
    resetDrag()
  }

  if (items.length === 0) return <p className={styles.queueEmpty}>Nothing queued</p>

  const draggedItem = dragKey ? (items.find((item) => getKey(item) === dragKey) ?? null) : null

  function isPointerOverTopZone(x: number, y: number): boolean {
    const rect = topZoneRef.current?.getBoundingClientRect()
    return rect ? x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom : false
  }

  function isPointerOverRemoveZone(x: number, y: number): boolean {
    const rect = removeZoneRef.current?.getBoundingClientRect()
    return rect ? x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom : false
  }

  return (
    <>
      <ol ref={listRef} className={`${styles.queueList}${dragKey ? ` ${styles.queueListDragging}` : ''}`}>
        {items.map((item) => {
          const key = getKey(item)
          const isDragging = dragKey === key
          const isPartner = draggedItem != null && partnersFor(draggedItem).some((partner) => getKey(partner) === key)
          const isDimmed = draggedItem != null && !isDragging && !isPartner
          const shiftY = !isDragging && draggedItem != null ? getShift(item, draggedItem, insertIdx) : 0
          const itemStyle = {
            '--item-color': getColor?.(item) ?? 'var(--border-subtle)',
          } as React.CSSProperties

          if (isDragging) {
            itemStyle.transform = `translate3d(0, ${dragDelta.toString()}px, 0) scale(1.03)`
            itemStyle.zIndex = 10
            itemStyle.position = 'relative'
            itemStyle.transition = 'none'
          } else if (shiftY !== 0) {
            itemStyle.transform = `translate3d(0, ${shiftY.toString()}px, 0)`
          }

          return (
            <li
              key={key}
              className={[
                styles.queueItem,
                canReorder ? '' : styles.queueItemStatic,
                isDragging ? styles.queueItemDragging : '',
                isDimmed ? styles.queueItemDim : '',
              ].filter(Boolean).join(' ')}
              style={itemStyle}
              ref={(el) => {
                if (el) itemElsRef.current.set(key, el)
                else itemElsRef.current.delete(key)
              }}
              onPointerDown={(e) => { handlePointerDown(e, item) }}
              onPointerMove={(e) => { handlePointerMove(e, item) }}
              onPointerUp={(e) => { handlePointerUp(e, item) }}
              onPointerCancel={(e) => { handlePointerCancel(e) }}
              onContextMenu={(e) => { if (canReorder) e.preventDefault() }}
            >
              {renderItem(item)}
              {renderActions?.(item)}
            </li>
          )
        })}
      </ol>
      {exitItems.map((exitItem) => (
        <div
          key={exitItem.key}
          className={`${styles.queueItem} ${styles.queueExitItem}`}
          style={{
            '--item-color': exitItem.color,
            top: exitItem.rect.top,
            left: exitItem.rect.left,
            width: exitItem.rect.width,
          } as React.CSSProperties}
          onAnimationEnd={() => {
            setExitItems((current) => current.filter((item) => item.key !== exitItem.key))
          }}
        >
          {renderItem(exitItem.item)}
          {renderActions?.(exitItem.item)}
        </div>
      ))}
      {dragKey && (onTopDrop != null || onRemoveDrop != null) && (
        <div className={styles.queueDropTargets}>
          {onTopDrop && (
            <div
              ref={topZoneRef}
              className={`${styles.queueDropTarget} ${styles.queueTopDrop}${topHot ? ` ${styles.queueTopDropHot}` : ''}`}
            >
              ↑
              <span>{topDropLabel}</span>
            </div>
          )}
          {onRemoveDrop && (
            <div
              ref={removeZoneRef}
              className={`${styles.queueDropTarget} ${styles.queueRemoveDrop}${removeHot ? ` ${styles.queueRemoveDropHot}` : ''}`}
            >
              ×
              <span>{removeDropLabel}</span>
            </div>
          )}
        </div>
      )}
    </>
  )
}
