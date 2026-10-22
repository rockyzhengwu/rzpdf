import unittest
from PIL import Image, ImageChops
from check_pf01_oracles import stripe_differences, radial_geometry


class OracleTests(unittest.TestCase):
    def test_known_packed_rows_pass_but_phase_shift_does_not(self):
        source=b''.join(bytes([255 if row%2==0 else 0])*8 for row in range(64))
        image=Image.frombytes('1',(64,64),source)
        self.assertEqual(stripe_differences(image,72)['differing_pixels'],0)
        self.assertEqual(stripe_differences(ImageChops.offset(image,0,1),72)['differing_pixels'],4096)

    def test_interpolated_blank_and_transparent_output_are_rejected(self):
        self.assertEqual(stripe_differences(Image.new('RGB',(64,64),(128,128,128)),72)['differing_pixels'],4096)
        self.assertEqual(stripe_differences(Image.new('RGB',(64,64),'white'),72)['differing_pixels'],2048)
        self.assertEqual(stripe_differences(Image.new('RGBA',(64,64),(0,0,0,0)),72)['differing_pixels'],4096)

    def test_wrong_geometry_is_rejected(self):
        with self.assertRaises(ValueError):stripe_differences(Image.new('RGB',(63,64)),72)

    def test_radial_oracle_is_outside_and_extends_past_one(self):
        from decimal import Decimal
        result=radial_geometry()
        self.assertGreater(Decimal(result['distance_squared_minus_one']),0)
        self.assertGreater(Decimal(result['end_center_largest_parameter']),1)
        self.assertFalse(result['outward_ray_coverage'])


if __name__=='__main__':unittest.main()
