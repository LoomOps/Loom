import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from host_metrics import cpu_usage,temperatures

class HostMetricsTests(unittest.TestCase):
    def test_cpu_delta_excludes_idle_and_iowait(self):
        old={'boot_id':'a','monotonic':10,'ticks':[100,0,100,500,50,0,0,0]}
        new={'boot_id':'a','monotonic':40,'ticks':[120,0,110,550,70,0,0,0]}
        self.assertEqual(cpu_usage(old,new),(30.0,30))
        new['boot_id']='b';self.assertEqual(cpu_usage(old,new),(None,None))
    def test_cpu_sensor_uses_package_and_ignores_disconnected_sensor(self):
        with TemporaryDirectory() as directory:
            chip=Path(directory)/'hwmon3';chip.mkdir();(chip/'name').write_text('k10temp')
            for n,label,value in ((1,'Tctl','46000'),(2,'Tdie','44000'),(3,'unconnected','-61000')):
                (chip/f'temp{n}_label').write_text(label);(chip/f'temp{n}_input').write_text(value)
            readings,cpu=temperatures(Path(directory))
        self.assertEqual(cpu,46);self.assertEqual(len(readings),2)

if __name__=='__main__':unittest.main()
