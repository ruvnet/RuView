import plotly.graph_objects as go

# Data from the provided JSON
data = {
    "wifi_same": {"AP": 43.5, "AP@50": 87.2, "AP@75": 44.6, "AP-m": 38.1, "AP-l": 46.4},
    "image_same": {"AP": 84.7, "AP@50": 94.4, "AP@75": 77.1, "AP-m": 70.3, "AP-l": 83.8},
    "wifi_diff": {"AP": 27.3, "AP@50": 51.8, "AP@75": 24.2, "AP-m": 22.1, "AP-l": 28.6}
}

# Extract metrics and values
metrics = list(data["wifi_same"].keys())
wifi_same_values = list(data["wifi_same"].values())
image_same_values = list(data["image_same"].values())
wifi_diff_values = list(data["wifi_diff"].values())

# Define colors from the brand palette - using darker color for WiFi Diff
colors = ['#1FB8CD', '#FFC185', '#5D878F']

# Create the grouped bar chart
fig = go.Figure()

# Add bars for each method with hover data
fig.add_trace(go.Bar(
    name='WiFi Same',
    x=metrics,
    y=wifi_same_values,
    marker_color=colors[0],
    hovertemplate='<b>WiFi Same</b><br>Metric: %{x}<br>Score: %{y}<extra></extra>'
))

fig.add_trace(go.Bar(
    name='Image Same',
    x=metrics,
    y=image_same_values,
    marker_color=colors[1],
    hovertemplate='<b>Image Same</b><br>Metric: %{x}<br>Score: %{y}<extra></extra>'
))

fig.add_trace(go.Bar(
    name='WiFi Diff',
    x=metrics,
    y=wifi_diff_values,
    marker_color=colors[2],
    hovertemplate='<b>WiFi Diff</b><br>Metric: %{x}<br>Score: %{y}<extra></extra>'
))

# Update layout
fig.update_layout(
    title='DensePose Performance Comparison',
    xaxis_title='AP Metrics',
    yaxis_title='Score',
    barmode='group',
    legend=dict(orientation='h', yanchor='bottom', y=1.05, xanchor='center', x=0.5),
    plot_bgcolor='rgba(0,0,0,0)',
    paper_bgcolor='white'
)

# Add grid for better readability
fig.update_yaxes(showgrid=True, gridcolor='lightgray')
fig.update_xaxes(showgrid=False)

# Save the chart
fig.write_image('densepose_performance_chart.png')