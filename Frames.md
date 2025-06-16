In the context of BBL (BlackBox Log) files, a **frame** is a **data record** that contains sensor readings and flight controller state at a specific point in time.

## What is a Frame?

A frame is essentially a **timestamped snapshot** of the flight controller's data at one moment during flight. Each frame contains multiple data fields like:

- **Gyroscope readings** (gyroADC[0], gyroADC[1], gyroADC[2])
- **Accelerometer readings** (accSmooth[0], accSmooth[1], accSmooth[2])
- **Motor outputs** (motor[0], motor[1], motor[2], motor[3])
- **RC commands** (rcCommand[0], rcCommand[1], rcCommand[2], rcCommand[3])
- **PID values**, **flight modes**, **battery voltage**, etc.

## Frame Types in BBL:

- **I frames (Intra)**: Complete data snapshots with all field values
- **P frames (Predictive)**: Compressed frames storing only changes from previous frame
- **H frames (GPS Home)**: GPS home position data
- **G frames (GPS)**: GPS position and speed data  
- **S frames (Slow)**: Less frequently updated data (battery, RSSI, etc.)
- **E frames (Event)**: Special events like mode changes, failsafes

## Data Structure:

```
Frame 1: [timestamp] [gyroX] [gyroY] [gyroZ] [motor1] [motor2] [motor3] [motor4] [rcThrottle] ...
Frame 2: [timestamp] [gyroX] [gyroY] [gyroZ] [motor1] [motor2] [motor3] [motor4] [rcThrottle] ...
Frame 3: [timestamp] [gyroX] [gyroY] [gyroZ] [motor1] [motor2] [motor3] [motor4] [rcThrottle] ...
```

So when our parser reports **"82,845 P frames"**, it means there are 82,845 individual data records captured during that flight, each containing a complete set of sensor readings and flight controller states at different moments in time.

**Think of it like:** Each frame = one row in a flight data spreadsheet, recorded hundreds or thousands of times per second during flight.
