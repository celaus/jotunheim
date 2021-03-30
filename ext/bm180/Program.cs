using System;
using System.Collections.Generic;
using System.Device.I2c;
using Iot.Device.Bmp180;
using Iot.Device.Common;
using Newtonsoft.Json;
using UnitsNet;

namespace x5ff.Bmp180Reader
{
    class JsonReading
    {
        public string unit { get; set; }
        public double value { get; set; }
        public string kind { get; set; }
        public string accessory_type { get; set; }


    }
    class Program
    {
        static void Main(string[] args)
        {
            const int busId = 1;
            I2cConnectionSettings i2cSettings = new(busId, Bmp180.DefaultI2cAddress);
            I2cDevice i2cDevice = I2cDevice.Create(i2cSettings);
            using (var i2CBmp180 = new Bmp180(i2cDevice))
            {
                var temperature = i2CBmp180.ReadTemperature().DegreesCelsius;
                var pressure = i2CBmp180.ReadPressure().Hectopascals;
                var readings = new List<JsonReading>() {
                    new JsonReading() {
                        kind = "temperature",
                        value = temperature,
                        unit = "celsius",
                        accessory_type = "Temperature"
                },  new JsonReading() {
                        kind = "pressure",
                        value = pressure,
                        unit = "hpa",
                        accessory_type = "Pressure"
                }};
                Console.WriteLine(JsonConvert.SerializeObject(readings));
            }
        }
    }
}
