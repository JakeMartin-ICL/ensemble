import React, { useEffect, useRef, useState } from 'react'
import styles from '../pages/car/Weave.module.css'

interface PositionedItem {
  position: number
}

export default function QueueList<T extends PositionedItem>({
  items,
  getKey,
  getGroupKey,
  getColor,
  renderItem,
  renderActions,
  onReorder,
  onRemoveDrop,
  canReorder = true,
  reorderScope = 'all',
  removeDropLabel = 'Remove',
}: {
  items: T[]
  getKey: (item: T) => string
  getGroupKey?: (item: T) => string
  getColor?: (item: T) => string
  renderItem: (item: T) => React.ReactNode
  renderActions?: (item: T) => React.ReactNode
  onReorder: (item: T, toPosition: number) => void
  onRemoveDrop?: (item: T) => void
  canReorder?: boolean
  reorderScope?: 'all' | 'group'
  removeDropLabel?: string
}) {
  const [dragKey, setDragKey] = useState<string | null>(null)
  const [dragDelta, setDragDelta] = useState(0)
  const [insertIdx, setInsertIdx] = useState(0)
  const [removeHot, setRemoveHot] = useState(false)

  const itemElsRef = useRef<Map<string, HTMLElement>>(new Map())
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
  const scrollAtStartRef = useRef(0)

  dragKeyRef.current = dragKey

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
    setRemoveHot(false)
    stopAutoScroll()
  }

  useEffect(() => resetDrag, [])

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
      if (draggedItem && onRemoveDrop && isPointerOverRemoveZone(e.clientX, e.clientY)) {
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

  function isPointerOverRemoveZone(x: number, y: number): boolean {
    const rect = removeZoneRef.current?.getBoundingClientRect()
    return rect ? x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom : false
  }

  return (
    <>
      <ol className={`${styles.queueList}${dragKey ? ` ${styles.queueListDragging}` : ''}`}>
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
            itemStyle.transform = `translateY(${dragDelta.toString()}px) scale(1.03)`
            itemStyle.zIndex = 10
            itemStyle.position = 'relative'
            itemStyle.transition = 'none'
          } else if (shiftY !== 0) {
            itemStyle.transform = `translateY(${shiftY.toString()}px)`
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
            >
              {renderItem(item)}
              {renderActions?.(item)}
            </li>
          )
        })}
      </ol>
      {dragKey && onRemoveDrop && (
        <div
          ref={removeZoneRef}
          className={`${styles.queueRemoveDrop}${removeHot ? ` ${styles.queueRemoveDropHot}` : ''}`}
        >
          ×
          <span>{removeDropLabel}</span>
        </div>
      )}
    </>
  )
}
