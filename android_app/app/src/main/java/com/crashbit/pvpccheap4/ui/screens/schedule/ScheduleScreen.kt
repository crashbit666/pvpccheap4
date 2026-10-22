package com.crashbit.pvpccheap4.ui.screens.schedule

import com.crashbit.pvpccheap4.R
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import com.crashbit.pvpccheap4.ui.formatDayTitle
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Divider
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import java.time.LocalDate
import java.time.LocalTime

data class ScheduleItem(
    val startHour: Int,
    val endHour: Int,
    val deviceName: String,
    val status: ScheduleStatus,
    val price: String? = null
)

enum class ScheduleStatus {
    COMPLETED_OFF,
    COMPLETED_ON,
    FAILED,
    RETRYING,
    MISSED,
    DONE,
    PENDING,
    EXECUTING
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ScheduleScreen(
    viewModel: ScheduleViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()

    // Refresh data when screen becomes visible
    LaunchedEffect(Unit) {
        viewModel.refresh()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.main_schedule_title)) },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent
                )
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            // Date selector
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                IconButton(onClick = { viewModel.previousDay() }) {
                    Icon(
                        painterResource(R.drawable.ic_chevron_left),
                        contentDescription = stringResource(R.string.main_schedule_previous_day)
                    )
                }

                Text(
                    text = formatDayTitle(uiState.selectedDate),
                    style = MaterialTheme.typography.titleMedium
                )

                IconButton(onClick = { viewModel.nextDay() }) {
                    Icon(
                        painterResource(R.drawable.ic_chevron_right),
                        contentDescription = stringResource(R.string.main_schedule_next_day)
                    )
                }
            }

            HorizontalDivider(
                modifier = Modifier.padding(horizontal = 16.dp),
                color = MaterialTheme.colorScheme.primary.copy(alpha = 0.5f)
            )

            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(top = 8.dp)
            ) {
                if (uiState.isLoading) {
                    CircularProgressIndicator(
                        modifier = Modifier.align(Alignment.Center)
                    )
                } else if (uiState.scheduleItems.isEmpty()) {
                    EmptyScheduleMessage(
                        modifier = Modifier.align(Alignment.Center)
                    )
                } else {
                    val listState = rememberLazyListState()

                    // Today opens at the current hour, with what already ran above
                    LaunchedEffect(uiState.selectedDate, uiState.scheduleItems.size) {
                        val index = if (uiState.selectedDate == LocalDate.now()) {
                            uiState.scheduleItems
                                .indexOfFirst { it.startHour >= LocalTime.now().hour }
                                .let { if (it < 0) uiState.scheduleItems.lastIndex else it }
                        } else 0
                        listState.scrollToItem(index)
                    }

                    LazyColumn(
                        state = listState,
                        contentPadding = PaddingValues(16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp)
                    ) {
                        items(uiState.scheduleItems) { item ->
                            ScheduleItemCard(item)
                        }
                    }
                }

                uiState.error?.let { error ->
                    Text(
                        text = error,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier
                            .align(Alignment.TopCenter)
                            .padding(16.dp)
                    )
                }
            }
        }
    }
}

@Composable
fun ScheduleItemCard(item: ScheduleItem) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
        )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Time column
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier.width(60.dp)
            ) {
                Text(
                    text = String.format("%02d:00", item.startHour),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold
                )
                Text(
                    text = String.format("%02d:00", item.endHour),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Spacer(modifier = Modifier.width(16.dp))

            // Device name and price
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = item.deviceName,
                    style = MaterialTheme.typography.bodyLarge
                )
                item.price?.let { price ->
                    Text(
                        text = price,
                        style = MaterialTheme.typography.bodySmall,
                        color = Color(0xFF4CAF50)
                    )
                }
            }

            Spacer(modifier = Modifier.width(16.dp))

            // Status badge
            StatusBadge(status = item.status)
        }
    }
}

@Composable
fun StatusBadge(status: ScheduleStatus) {
    val (text, backgroundColor, textColor) = when (status) {
        ScheduleStatus.COMPLETED_OFF -> Triple(stringResource(R.string.main_status_off), Color(0xFF4CAF50).copy(alpha = 0.2f), Color(0xFF4CAF50))
        ScheduleStatus.COMPLETED_ON -> Triple(stringResource(R.string.main_status_on), Color(0xFF2196F3).copy(alpha = 0.2f), Color(0xFF2196F3))
        ScheduleStatus.FAILED -> Triple(stringResource(R.string.main_status_failed), Color(0xFFF44336).copy(alpha = 0.2f), Color(0xFFF44336))
        ScheduleStatus.RETRYING -> Triple(stringResource(R.string.main_status_retrying), Color(0xFFFF9800).copy(alpha = 0.2f), Color(0xFFFF9800))
        ScheduleStatus.MISSED -> Triple(stringResource(R.string.main_status_missed), Color(0xFF9E9E9E).copy(alpha = 0.2f), Color(0xFF757575))
        ScheduleStatus.DONE -> Triple(stringResource(R.string.main_status_done), Color(0xFF4CAF50).copy(alpha = 0.1f), Color(0xFF4CAF50))
        ScheduleStatus.PENDING -> Triple(stringResource(R.string.main_status_pending), Color(0xFF9E9E9E).copy(alpha = 0.2f), Color(0xFF9E9E9E))
        ScheduleStatus.EXECUTING -> Triple(stringResource(R.string.main_status_executing), Color(0xFF2196F3).copy(alpha = 0.1f), Color(0xFF2196F3))
    }

    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(16.dp))
            .background(backgroundColor)
            .padding(horizontal = 12.dp, vertical = 6.dp)
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.labelMedium,
            color = textColor,
            fontWeight = FontWeight.Medium
        )
    }
}

@Composable
fun EmptyScheduleMessage(modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = stringResource(R.string.main_schedule_empty_title),
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = stringResource(R.string.main_schedule_empty_body),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
        )
    }
}
