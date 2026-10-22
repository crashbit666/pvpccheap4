package com.crashbit.pvpccheap4.ui

import android.text.format.DateFormat
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale

/** 0.1234 → "0,1234 €/kWh" (ca/es) or "0.1234 €/kWh" (en): decimal separator of the device locale. */
fun formatPrice(price: Double): String = String.format(Locale.getDefault(), "%.4f €/kWh", price)

/** Weekday, day and month in the device language, e.g. "dijous, 22 d’octubre" or "Thursday, October 22". */
fun formatDayTitle(date: LocalDate, locale: Locale = Locale.getDefault()): String {
    val pattern = DateFormat.getBestDateTimePattern(locale, "EEEEdMMMM")
    return date.format(DateTimeFormatter.ofPattern(pattern, locale))
}
